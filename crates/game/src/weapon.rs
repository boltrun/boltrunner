use bevy::{
    camera::{Exposure, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    ecs::system::SystemParam,
    light::NotShadowCaster,
    prelude::*,
    scene::SceneInstanceReady,
    transform::helper::TransformHelper,
};
use boltrunner_client::{PredictedShot, PredictedShots};
use boltrunner_shared::{
    PlayerId, PlayerState,
    arsenal::WeaponKind,
    level,
    weapon::{Projectile, ShotImpact},
};
use lightyear::prelude::*;
use std::collections::VecDeque;

const VIEW_LAYER: usize = 1;

const MUZZLE_CLEARANCE: f32 = 0.015;
const FLASH_HALF_LENGTH: f32 = 0.055;

pub struct WeaponPlugin;
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct WeaponVisuals;
impl Plugin for WeaponPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Feedback>()
            .init_resource::<MuzzleGeometry>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                    detect_shots,
                    animate_weapon,
                    local_shots,
                    attach_bolts,
                    receive_impacts,
                    reconcile_bolts,
                    advance_local_bolts,
                    sync_bolts,
                    fade_impacts,
                    hit_feedback,
                )
                    .chain()
                    .in_set(WeaponVisuals)
                    .after(InterpolationSystems::Interpolate)
                    .after(crate::world_weapon::WorldWeaponsUpdated),
            );
    }
}

#[derive(Resource)]
pub(crate) struct WeaponAssets {
    pub scenes: Vec<Handle<Scene>>,
    pub drone: Handle<Scene>,
    beam: Handle<Mesh>,
    spark: Handle<Mesh>,
    cyan: Handle<StandardMaterial>,
    core: Handle<StandardMaterial>,
    glow: Handle<StandardMaterial>,
    seeker_core: Handle<StandardMaterial>,
    seeker_glow: Handle<StandardMaterial>,
    colors: Vec<Handle<StandardMaterial>>,
    rocket_body: Handle<Mesh>,
    rocket_nose: Handle<Mesh>,
    rocket_metal: Handle<StandardMaterial>,
}
#[derive(Resource, Default)]
struct Feedback {
    player: Option<u64>,
    recoil: f32,
    flash: f32,
    pending_shots: Vec<PredictedShot>,
    seen: VecDeque<(u64, u16, u8, f64)>,
    finished: VecDeque<(u64, u16, u8, f64)>,
    impacts: Vec<(ShotImpact, f64)>,
    hit_flash: f32,
}
#[derive(Resource, Default)]
pub(crate) struct MuzzleGeometry(pub Option<(Vec3, Vec3, Vec3)>);
#[derive(Component)]
struct ViewWeapon(WeaponKind);
#[derive(Component)]
struct MuzzleFlash;
#[derive(Component)]
struct MuzzleSocket(WeaponKind);
#[derive(Component)]
struct WeaponCamera;
#[derive(Component)]
struct BoltVisual;
#[derive(Component)]
struct LocalBolt {
    bolt: Projectile,
    age: f32,
    muzzle_offset: Vec3,
    correction: Vec3,
    confirmed_tick: Option<Tick>,
    accumulated_time: f64,
}
#[derive(Component)]
struct ImpactFlash {
    age: f32,
}
#[derive(Component)]
struct ModelPart {
    kind: WeaponKind,
    name: String,
    rest: Transform,
}

fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let scene = assets.load(
        GltfAssetLabel::Scene(0).from_asset("weapons/starter_pistol/built/starter_pistol.glb"),
    );
    let mut scenes = vec![scene.clone()];
    for name in [
        "flamethrower",
        "shotgun",
        "machine_gun",
        "rocket_launcher",
        "laser",
        "sniper",
        "drone_controller",
        "ricochet",
        "karambit",
        "arc_cannon",
        "plasma_launcher",
        "electro_plasma",
        "seeker_plasma",
    ] {
        scenes.push(assets.load(
            GltfAssetLabel::Scene(0).from_asset(format!("weapons/arsenal/built/{name}.gltf")),
        ));
    }
    let colors = WeaponKind::ALL
        .into_iter()
        .map(|kind| {
            let [r, g, b] = kind.color();
            materials.add(StandardMaterial {
                base_color: Color::srgb(r, g, b),
                emissive: LinearRgba::new(r * 2.0, g * 2.0, b * 2.0, 1.0),
                unlit: true,
                ..default()
            })
        })
        .collect();
    let weapon = WeaponAssets {
        scenes,
        colors,
        rocket_body: meshes.add(Cylinder::new(1.0, 1.0)),
        rocket_nose: meshes.add(Cone::new(1.0, 1.0)),
        rocket_metal: materials.add(StandardMaterial {
            base_color_texture: Some(assets.load("weapons/arsenal/built/metal.png")),
            metallic: 0.65,
            perceptual_roughness: 0.4,
            ..default()
        }),
        drone: assets.load(GltfAssetLabel::Scene(0).from_asset("weapons/arsenal/built/drone.gltf")),
        beam: meshes.add(Cuboid::new(1., 1., 1.)),
        spark: meshes.add(Sphere::new(1.).mesh().ico(2).unwrap()),
        cyan: materials.add(StandardMaterial {
            base_color: Color::srgb(0.45, 0.85, 1.),
            unlit: true,
            ..default()
        }),
        core: materials.add(StandardMaterial {
            base_color: Color::srgb(0.82, 0.97, 1.),
            unlit: true,
            ..default()
        }),
        glow: materials.add(StandardMaterial {
            base_color: Color::srgba(0.25, 0.72, 1., 0.5),
            unlit: true,
            alpha_mode: AlphaMode::Add,
            ..default()
        }),
        seeker_core: materials.add(StandardMaterial {
            base_color: Color::srgb(0.82, 1.0, 0.76),
            unlit: true,
            ..default()
        }),
        seeker_glow: materials.add(StandardMaterial {
            base_color: {
                let [r, g, b] = WeaponKind::HomingPlasma.color();
                Color::srgba(r, g, b, 0.5)
            },
            unlit: true,
            alpha_mode: AlphaMode::Add,
            ..default()
        }),
    };
    // A separate depth buffer keeps the first-person gun out of nearby walls.
    // This camera also draws the existing crosshair/FPS UI above the weapon.
    commands.spawn((
        Camera3d::default(),
        WeaponCamera,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        Projection::Perspective(PerspectiveProjection {
            fov: 70_f32.to_radians(),
            near: 0.01,
            ..default()
        }),
        Exposure::INDOOR,
        // Keep weapon fill independent of the map. The coated materials limit
        // black-paint reflections while this fill keeps the surface wear visible.
        AmbientLight {
            brightness: 450.,
            ..default()
        },
        Tonemapping::None,
        Transform::default(),
        RenderLayers::layer(VIEW_LAYER),
        Msaa::Sample4,
        IsDefaultUiCamera,
    ));
    for kind in WeaponKind::ALL {
        commands
            .spawn((
                SceneRoot(weapon.scenes[kind.index()].clone()),
                Transform::from_translation(Vec3::new(0.23, -0.20, -0.46)),
                Visibility::Hidden,
                ViewWeapon(kind),
            ))
            .observe(prepare_view_model);
    }
    for (position, intensity, color) in [
        (Vec3::new(-0.5, 0.7, 0.2), 35., Color::srgb(0.8, 0.9, 1.)),
        (Vec3::new(0.8, 0.1, -0.5), 20., Color::srgb(0.5, 0.75, 1.)),
    ] {
        commands.spawn((
            PointLight {
                intensity,
                color,
                shadows_enabled: false,
                range: 3.,
                ..default()
            },
            Transform::from_translation(position),
            RenderLayers::layer(VIEW_LAYER),
        ));
    }
    commands.insert_resource(weapon);
}

fn prepare_view_model(
    event: On<SceneInstanceReady>,
    children: Query<&Children>,
    names: Query<&Name>,
    mut commands: Commands,
    assets: Res<WeaponAssets>,
    roots: Query<&ViewWeapon>,
    transforms: Query<&Transform>,
) {
    let Ok(root) = roots.get(event.entity) else {
        return;
    };
    for entity in children.iter_descendants(event.entity) {
        commands
            .entity(entity)
            .insert((RenderLayers::layer(VIEW_LAYER), NotShadowCaster));
        if names
            .get(entity)
            .is_ok_and(|name| name.as_str() == "Muzzle")
        {
            commands
                .entity(entity)
                .insert(MuzzleSocket(root.0))
                .with_child((
                    Mesh3d(assets.spark.clone()),
                    MeshMaterial3d(assets.core.clone()),
                    // The sphere's rear edge must also clear the barrel lip.
                    Transform::from_xyz(0., 0., -(MUZZLE_CLEARANCE + FLASH_HALF_LENGTH))
                        .with_scale(Vec3::new(0.03, 0.03, FLASH_HALF_LENGTH)),
                    Visibility::Hidden,
                    RenderLayers::layer(VIEW_LAYER),
                    NotShadowCaster,
                    MuzzleFlash,
                ));
        }
        if let Ok(name) = names.get(entity) {
            if matches!(
                name.as_str(),
                "MovingBolt"
                    | "MovingPump"
                    | "ChargeCore"
                    | "HeatStrip"
                    | "CoolingVentLeft"
                    | "CoolingVentRight"
                    | "FuelTank"
                    | "AmmoDrum"
                    | "CellMagazine"
                    | "BallChamber"
                    | "RearCapacitor"
                    | "Capacitor"
                    | "Display"
            ) || crate::reload_animation::is_part(root.0, name.as_str())
            {
                if let Ok(rest) = transforms.get(entity) {
                    commands.entity(entity).insert(ModelPart {
                        kind: root.0,
                        name: name.as_str().to_owned(),
                        rest: *rest,
                    });
                }
            }
        }
    }
}

fn beam_children(
    parent: &mut ChildSpawnerCommands,
    assets: &WeaponAssets,
    projectiles: &crate::projectile_visual::ProjectileAssets,
    bolt: &Projectile,
) {
    if crate::projectile_visual::spawn(parent, projectiles, bolt) {
        return;
    }
    if matches!(bolt.kind, WeaponKind::Rocket | WeaponKind::Grenade) {
        if bolt.kind == WeaponKind::Rocket {
            parent.spawn((
                Mesh3d(assets.rocket_body.clone()),
                MeshMaterial3d(assets.rocket_metal.clone()),
                Transform::from_xyz(0.0, 0.0, bolt.radius * 1.1)
                    .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
                    .with_scale(Vec3::new(
                        bolt.radius * 0.7,
                        bolt.radius * 2.0,
                        bolt.radius * 0.7,
                    )),
                NotShadowCaster,
            ));
            parent.spawn((
                Mesh3d(assets.rocket_nose.clone()),
                MeshMaterial3d(assets.colors[bolt.kind.index()].clone()),
                Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
                    .with_scale(Vec3::new(
                        bolt.radius * 0.7,
                        bolt.radius * 1.5,
                        bolt.radius * 0.7,
                    )),
                NotShadowCaster,
            ));
        } else {
            parent.spawn((
                Mesh3d(assets.spark.clone()),
                MeshMaterial3d(assets.colors[bolt.kind.index()].clone()),
                Transform::from_scale(Vec3::splat(bolt.radius)),
                NotShadowCaster,
            ));
            if bolt.kind == WeaponKind::Grenade {
                parent.spawn((
                    Mesh3d(assets.spark.clone()),
                    MeshMaterial3d(assets.core.clone()),
                    Transform::from_scale(Vec3::splat(bolt.radius * 0.55)),
                    NotShadowCaster,
                ));
                parent.spawn((
                    Mesh3d(assets.rocket_body.clone()),
                    MeshMaterial3d(assets.rocket_metal.clone()),
                    Transform::from_rotation(Quat::from_rotation_z(0.7)).with_scale(Vec3::new(
                        bolt.radius * 2.0,
                        bolt.radius * 0.2,
                        bolt.radius * 2.0,
                    )),
                    NotShadowCaster,
                ));
            }
        }
        if bolt.kind == WeaponKind::Rocket {
            parent.spawn((
                Mesh3d(assets.beam.clone()),
                MeshMaterial3d(assets.core.clone()),
                Transform::from_xyz(0.0, 0.0, 0.22).with_scale(Vec3::new(0.055, 0.055, 0.3)),
                NotShadowCaster,
            ));
        }
        return;
    }
    if bolt.kind != WeaponKind::Pistol {
        parent.spawn((
            Mesh3d(assets.beam.clone()),
            MeshMaterial3d(assets.colors[bolt.kind.index()].clone()),
            Transform::from_scale(Vec3::new(bolt.radius * 2.0, bolt.radius * 2.0, 1.0)),
            NotShadowCaster,
        ));
        return;
    }
    for (width, material) in [
        (0.16, &assets.glow),
        (0.08, &assets.cyan),
        (0.0325, &assets.core),
    ] {
        parent.spawn((
            Mesh3d(assets.beam.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_scale(Vec3::new(width, width, 1.)),
            NotShadowCaster,
        ));
    }
}

/// Read the authored socket through the current hierarchy, including recoil.
/// Both cameras share a viewport but have different FOVs: convert the socket
/// through their projection scales before placing a world-space projectile.
#[derive(SystemParam)]
struct MuzzleView<'w, 's> {
    sockets: Query<'w, 's, (Entity, &'static MuzzleSocket)>,
    local: Query<'w, 's, &'static PlayerState, With<Predicted>>,
    transforms: TransformHelper<'w, 's>,
    world_camera:
        Query<'w, 's, (&'static Transform, &'static Projection), With<crate::view::PlayerCamera>>,
    weapon_camera: Query<'w, 's, &'static Projection, With<WeaponCamera>>,
}

impl MuzzleView<'_, '_> {
    fn shot_geometry(&self) -> Option<(Vec3, Vec3, Vec3)> {
        let selected = self.local.single().ok()?.weapon.selected;
        let entity = self
            .sockets
            .iter()
            .find(|(_, socket)| socket.0 == selected)?
            .0;
        let socket = self
            .transforms
            .compute_global_transform(entity)
            .ok()?
            .transform_point(Vec3::NEG_Z * MUZZLE_CLEARANCE);
        let (camera, Projection::Perspective(world_projection)) =
            self.world_camera.single().ok()?
        else {
            return None;
        };
        let Projection::Perspective(weapon_projection) = self.weapon_camera.single().ok()? else {
            return None;
        };
        let scale = (world_projection.fov * 0.5).tan() / (weapon_projection.fov * 0.5).tan();
        let position = camera.transform_point(socket * Vec3::new(scale, scale, 1.));
        Some((position, camera.translation, *camera.forward()))
    }
}

fn detect_shots(
    mut commands: Commands,
    session: Res<boltrunner_client::Session>,
    effects: Query<Entity, Or<(With<LocalBolt>, With<ImpactFlash>)>>,
    settings: Res<boltrunner_client::tuning::TuningClient>,
    mut feedback: ResMut<Feedback>,
    mut shots: ResMut<PredictedShots>,
    time: Res<Time<Real>>,
    players: Query<(&PlayerId, &PlayerState), With<Predicted>>,
) {
    if !session.is_playing() {
        for entity in &effects {
            commands.entity(entity).despawn();
        }
        *feedback = default();
        shots.0.clear();
        return;
    }
    feedback.pending_shots.clear();
    let now = time.elapsed_secs_f64();
    feedback.seen.retain(|(_, _, _, at)| now - at < 60.0);
    feedback.finished.retain(|(_, _, _, at)| now - at < 60.0);
    let Ok((id, state)) = players.single() else {
        *feedback = default();
        shots.0.clear();
        return;
    };
    if feedback.player != Some(id.0) {
        feedback.seen.clear();
        feedback.finished.clear();
        feedback.impacts.clear();
        feedback.player = Some(id.0);
    }
    for shot in shots.0.drain(..) {
        if shot.bolt.owner != id.0
            || feedback.seen.iter().any(|(owner, number, pellet, _)| {
                *owner == id.0 && *number == shot.bolt.shot && *pellet == shot.bolt.pellet
            })
        {
            continue;
        }
        feedback
            .seen
            .push_back((id.0, shot.bolt.shot, shot.bolt.pellet, now));
        if state.death.is_none() {
            feedback.pending_shots.push(shot);
            feedback.recoil = 1.0;
            feedback.flash = settings.values().presentation.muzzle_flash_time;
        }
    }
}

fn local_shots(
    projectiles: Res<crate::projectile_visual::ProjectileAssets>,
    mut commands: Commands,
    assets: Res<WeaponAssets>,
    mut feedback: ResMut<Feedback>,
    timeline: Res<LocalTimeline>,
    muzzle: MuzzleView,
    mut muzzle_geometry: ResMut<MuzzleGeometry>,
    targets: Query<(&PlayerId, &PlayerState)>,
) {
    let targets: Vec<_> = targets.iter().map(|(id, state)| (id.0, state)).collect();
    let geometry = muzzle.shot_geometry();
    muzzle_geometry.0 = geometry;
    for shot in feedback.pending_shots.drain(..) {
        let mut bolt = shot.bolt;
        if !bolt.kind.is_projectile() {
            continue;
        }
        let visual = bolt.clone();
        let age = ((timeline.tick() - shot.tick) as f32).max(0.0)
            * boltrunner_shared::TICK_DURATION.as_secs_f32();
        if age >= bolt.lifetime_ticks as f32 * boltrunner_shared::TICK_DURATION.as_secs_f32() {
            continue;
        }
        let muzzle_offset = geometry.map_or(Vec3::ZERO, |(muzzle, _, _)| muzzle - bolt.origin);
        let ticks = (age / boltrunner_shared::TICK_DURATION.as_secs_f32()).round() as u16;
        if !predict_wall_steps(&mut bolt, ticks, &targets) {
            continue;
        }
        commands
            .spawn((
                LocalBolt {
                    bolt,
                    age,
                    muzzle_offset,
                    correction: Vec3::ZERO,
                    confirmed_tick: None,
                    accumulated_time: 0.0,
                },
                Transform::default(),
                Visibility::Inherited,
            ))
            .with_children(|parent| beam_children(parent, &assets, &projectiles, &visual));
    }
}

fn predict_wall_steps(bolt: &mut Projectile, steps: u16, targets: &[(u64, &PlayerState)]) -> bool {
    let dt = boltrunner_shared::TICK_DURATION.as_secs_f32();
    for _ in 0..steps {
        if bolt.remaining_ticks == 0 {
            return false;
        }
        bolt.guide(level::world(), targets.iter().copied(), dt);
        bolt.apply_gravity(dt);
        let mut left = dt;
        for _ in 0..4 {
            let delta = bolt.direction * bolt.speed * left;
            if let Some(hit) =
                boltrunner_shared::weapon::advance_map_hit_moving(bolt, delta, std::iter::empty())
            {
                if !bolt.kind.rebounds() || bolt.bounces == 0 {
                    // Homing uses interpolated targets, so its predicted path can
                    // disagree with the server near a wall. Wait at the surface
                    // until a confirmed projectile or impact arrives.
                    if bolt.kind == WeaponKind::HomingPlasma {
                        return true;
                    }
                    return false;
                }
                bolt.position += hit.normal * 0.005;
                bolt.direction = (bolt.direction
                    - 2.0 * bolt.direction.dot(hit.normal) * hit.normal)
                    .normalize_or_zero();
                bolt.speed *= bolt.bounce_loss;
                bolt.bounces -= 1;
                bolt.can_hit_owner = true;
                left *= 1.0 - hit.fraction;
                if left < 1e-6 {
                    break;
                }
            } else {
                break;
            }
        }
        bolt.remaining_ticks -= 1;
    }
    bolt.remaining_ticks > 0
}

type UnrenderedBolt = (With<Projectile>, With<Interpolated>, Without<BoltVisual>);

fn attach_bolts(
    projectiles: Res<crate::projectile_visual::ProjectileAssets>,
    mut commands: Commands,
    assets: Res<WeaponAssets>,
    bolts: Query<(Entity, &Projectile), UnrenderedBolt>,
) {
    for (entity, bolt) in &bolts {
        commands
            .entity(entity)
            .insert((BoltVisual, Transform::default(), Visibility::Inherited))
            .with_children(|parent| beam_children(parent, &assets, &projectiles, bolt));
    }
}

fn bolt_transform(
    bolt: &Projectile,
    tuning: boltrunner_shared::tuning::PresentationTuning,
) -> Transform {
    let distance = bolt.position.distance(bolt.origin);
    if matches!(
        bolt.kind,
        WeaponKind::Rocket
            | WeaponKind::Ricochet
            | WeaponKind::Grenade
            | WeaponKind::ElectroPlasma
            | WeaponKind::HomingPlasma
    ) {
        return Transform::from_translation(bolt.position).looking_to(bolt.direction, Vec3::Y);
    }
    let length = distance.clamp(
        0.02,
        if bolt.kind == WeaponKind::Pistol {
            tuning.beam_length
        } else {
            0.6
        },
    );
    Transform::from_translation(bolt.position - bolt.direction * length / 2.)
        .looking_to(bolt.direction, Vec3::Y)
        .with_scale(Vec3::new(tuning.beam_width, tuning.beam_width, length))
}

fn receive_impacts(
    settings: Res<boltrunner_client::tuning::TuningClient>,
    mut commands: Commands,
    time: Res<Time<Real>>,
    assets: Res<WeaponAssets>,
    mut feedback: ResMut<Feedback>,
    mut blast_view: ResMut<crate::arsenal::DroneBlastView>,
    muzzle: Res<MuzzleGeometry>,
    mut receivers: Query<&mut MessageReceiver<ShotImpact>, With<Connected>>,
    timelines: Query<&lightyear::interpolation::timeline::InterpolationTimeline, With<Connected>>,
    locals: Query<(Entity, &LocalBolt)>,
) {
    let now = time.elapsed_secs_f64();
    for mut receiver in &mut receivers {
        for impact in receiver.receive() {
            // Matching is by shot identity, including hits before the server
            // projectile was ever replicated.
            for (entity, local) in &locals {
                if impact.terminal
                    && (local.bolt.owner, local.bolt.shot, local.bolt.pellet)
                        == (impact.owner, impact.shot, impact.pellet)
                {
                    commands.entity(entity).despawn();
                }
            }
            if feedback.player == Some(impact.owner) && impact.victim.is_some() {
                feedback.hit_flash = settings.values().presentation.hit_marker_time;
            }
            feedback.impacts.push((impact, now));
        }
    }
    let tick = timelines.iter().next().map(|timeline| timeline.tick());
    let owner = feedback.player;
    let mut completed = Vec::new();
    feedback.impacts.retain(|(impact, received)| {
        // Other players and their splashes use the same presentation time.
        if owner == Some(impact.owner)
            || tick.is_some_and(|tick| tick >= Tick(impact.tick))
            || now - received > 0.5
        {
            if impact.explosion_radius > 0.0 {
                crate::arsenal::spawn_explosion(&mut commands, impact);
                if impact.kind == WeaponKind::Drone && owner == Some(impact.owner) {
                    let duration = settings.values().presentation.drone_blast_view_time;
                    *blast_view = crate::arsenal::DroneBlastView {
                        center: impact.position,
                        remaining: duration,
                        duration,
                    };
                }
            } else if matches!(impact.kind, WeaponKind::Sniper | WeaponKind::Arc) {
                let mut ray = impact.clone();
                if impact.kind == WeaponKind::Arc
                    && impact.pellet == 0
                    && owner == Some(impact.owner)
                    && let Some((position, _, _)) = muzzle.0
                {
                    ray.origin = position;
                }
                crate::arsenal::spawn_ray(&mut commands, &ray);
            }
            if matches!(impact.kind, WeaponKind::Shotgun | WeaponKind::Karambit) {
                crate::arsenal::spawn_pellet_impact(&mut commands, impact);
            } else if !impact.kind.is_stream()
                && (impact.kind != WeaponKind::Sniper || impact.terminal)
                && !matches!(
                    impact.kind,
                    WeaponKind::Drone | WeaponKind::Arc | WeaponKind::Grenade
                )
            {
                spawn_impact(
                    &mut commands,
                    &assets,
                    impact.kind,
                    impact.position,
                    impact.normal,
                    settings.values().presentation,
                );
            }
            if impact.terminal {
                completed.push((impact.owner, impact.shot, impact.pellet, now));
            }
            false
        } else {
            true
        }
    });
    feedback.finished.extend(completed);
}

fn reconcile_bolts(
    mut commands: Commands,
    timeline: Res<LocalTimeline>,
    authoritative: Query<(&Confirmed<Projectile>, &ConfirmedTick), With<Interpolated>>,
    mut locals: Query<(Entity, &mut LocalBolt)>,
    targets: Query<(&PlayerId, &PlayerState)>,
) {
    let targets: Vec<_> = targets.iter().map(|(id, state)| (id.0, state)).collect();
    for (entity, mut local) in &mut locals {
        let Some((confirmed, tick)) = authoritative.iter().find(|(confirmed, _)| {
            (confirmed.0.owner, confirmed.0.shot, confirmed.0.pellet)
                == (local.bolt.owner, local.bolt.shot, local.bolt.pellet)
        }) else {
            continue;
        };
        if local.confirmed_tick == Some(tick.tick) {
            continue;
        }
        let lead = ((timeline.tick() - tick.tick) as f32).clamp(0.0, 30.0) as u16;
        let mut predicted = confirmed.0.clone();
        if !predict_wall_steps(&mut predicted, lead, &targets) {
            commands.entity(entity).despawn();
            continue;
        }
        let position = predicted.position;
        let relocated = local.bolt.relocation != predicted.relocation;
        let difference = local.bolt.position - position;
        local.correction = if !relocated && difference.length() < 3.0 {
            local.correction + difference
        } else {
            Vec3::ZERO
        };
        local.bolt = predicted;
        local.age = (local
            .bolt
            .lifetime_ticks
            .saturating_sub(local.bolt.remaining_ticks)) as f32
            * boltrunner_shared::TICK_DURATION.as_secs_f32();
        local.accumulated_time = 0.0;
        local.confirmed_tick = Some(tick.tick);
    }
}

fn advance_local_bolts(
    settings: Res<boltrunner_client::tuning::TuningClient>,
    mut commands: Commands,
    time: Res<Time>,
    mut bolts: Query<(Entity, &mut LocalBolt, &mut Transform)>,
    targets: Query<(&PlayerId, &PlayerState)>,
) {
    let targets: Vec<_> = targets.iter().map(|(id, state)| (id.0, state)).collect();
    for (entity, mut local, mut transform) in &mut bolts {
        let tick = boltrunner_shared::TICK_DURATION.as_secs_f64();
        local.accumulated_time += time.delta_secs_f64();
        let steps = (local.accumulated_time / tick)
            .floor()
            .min(local.bolt.remaining_ticks as f64) as u16;
        local.accumulated_time -= steps as f64 * tick;
        // Predicted wall occlusion is cosmetic. Player hits and impact splashes
        // are exclusively confirmed by the server.
        let relocation = local.bolt.relocation;
        if !predict_wall_steps(&mut local.bolt, steps, &targets) {
            commands.entity(entity).despawn();
            continue;
        }
        if relocation != local.bolt.relocation {
            local.correction = Vec3::ZERO;
        }
        local.age += steps as f32 * tick as f32;
        local.correction *= (-35.0 * time.delta_secs()).exp();
        if local.age
            >= local.bolt.lifetime_ticks as f32 * boltrunner_shared::TICK_DURATION.as_secs_f32()
        {
            commands.entity(entity).despawn();
            continue;
        }
        let muzzle_weight = if local.bolt.relocation == 0 {
            (1.0 - local.bolt.position.distance(local.bolt.origin)
                / settings.values().presentation.muzzle_blend_distance)
                .clamp(0.0, 1.0)
        } else {
            0.0
        };
        let offset = local.correction + local.muzzle_offset * muzzle_weight;
        *transform = bolt_transform(&local.bolt, settings.values().presentation);
        let lead = local.bolt.direction * local.bolt.speed * local.accumulated_time as f32;
        transform.translation += level::world().clip_camera(
            local.bolt.position,
            local.bolt.position + lead,
            local.bolt.radius,
        ) - local.bolt.position;
        transform.translation += offset;
    }
}

fn hit_feedback(
    time: Res<Time>,
    mut feedback: ResMut<Feedback>,
    mut crosshair: Single<&mut BackgroundColor, With<crate::view::Crosshair>>,
) {
    crosshair.0 = if feedback.hit_flash > 0.0 {
        Color::srgb(0.45, 0.85, 1.0)
    } else {
        Color::WHITE
    };
    feedback.hit_flash = (feedback.hit_flash - time.delta_secs()).max(0.0);
}

fn sync_bolts(
    world_muzzles: Res<crate::world_weapon::WorldMuzzles>,
    settings: Res<boltrunner_client::tuning::TuningClient>,
    feedback: Res<Feedback>,
    mut bolts: Query<(&Projectile, &mut Transform, &mut Visibility), With<BoltVisual>>,
) {
    for (bolt, mut transform, mut visibility) in &mut bolts {
        *visibility = if (feedback.player == Some(bolt.owner)
            && feedback.seen.iter().any(|(owner, shot, pellet, _)| {
                (*owner, *shot, *pellet) == (bolt.owner, bolt.shot, bolt.pellet)
            }))
            || feedback.finished.iter().any(|(owner, shot, pellet, _)| {
                (*owner, *shot, *pellet) == (bolt.owner, bolt.shot, bolt.pellet)
            }) {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        *transform = bolt_transform(bolt, settings.values().presentation);
        if feedback.player != Some(bolt.owner) {
            if let Some(muzzle) = world_muzzles.0.get(&bolt.owner) {
                let distance = bolt.position.distance(bolt.origin);
                let weight = (1.0
                    - distance
                        / settings
                            .values()
                            .presentation
                            .muzzle_blend_distance
                            .max(0.01))
                .clamp(0.0, 1.0);
                transform.translation += (*muzzle - bolt.origin) * weight;
            }
        }
    }
}

fn animate_weapon(
    settings: Res<boltrunner_client::tuning::TuningClient>,
    mut camera: Single<(&mut Projection, &mut AmbientLight), With<WeaponCamera>>,
    time: Res<Time>,
    mut feedback: ResMut<Feedback>,
    local: Query<&PlayerState, With<Predicted>>,
    mut guns: Query<(&ViewWeapon, &mut Transform, &mut Visibility), Without<ModelPart>>,
    mut parts: Query<
        (
            &ModelPart,
            &mut Transform,
            Option<&MeshMaterial3d<StandardMaterial>>,
            &mut Visibility,
        ),
        (Without<ViewWeapon>, Without<MuzzleFlash>),
    >,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mouse: Res<ButtonInput<MouseButton>>,
    blast_view: Res<crate::arsenal::DroneBlastView>,
    mut cooling: Local<f32>,
    mut flashes: Query<
        (&mut Visibility, &mut Transform),
        (With<MuzzleFlash>, Without<ViewWeapon>, Without<ModelPart>),
    >,
) {
    let tuning = settings.values().presentation;
    if let Projection::Perspective(projection) = &mut *camera.0 {
        projection.fov = tuning.weapon_fov.to_radians();
    }
    camera.1.brightness = tuning.weapon_brightness;
    let state = local.single().ok();
    let alive = blast_view.remaining <= 0.0
        && state.is_some_and(|state| {
            state.death.is_none() && state.drone.is_none_or(|d| !d.controlling)
        });
    let selected = state.map_or(WeaponKind::Pistol, |s| s.weapon.selected);
    let cooling_target =
        if selected == WeaponKind::MachineGun && state.is_some_and(|s| s.weapon.overheated) {
            1.0
        } else {
            0.0
        };
    *cooling += (cooling_target - *cooling) * (1.0 - (-12.0 * time.delta_secs()).exp());
    let kick = feedback.recoil
        * match selected {
            WeaponKind::Shotgun => 1.7,
            WeaponKind::Rocket => 1.4,
            WeaponKind::Sniper => 1.6,
            WeaponKind::MachineGun => 0.55,
            WeaponKind::Karambit => 0.0,
            WeaponKind::Arc => 0.45,
            WeaponKind::Grenade => 1.25,
            WeaponKind::Flame | WeaponKind::Laser => 0.15,
            _ => 1.0,
        };
    let switch = state.map_or(0.0, |s| {
        s.weapon.switch_ticks as f32
            / (s.tuning.weapon_switch_time * boltrunner_shared::TICK_HZ as f32).max(1.0)
    });
    let reloading = state.is_some_and(|s| s.weapon.reload_ticks > 0);
    let reload_progress = state.map_or(0.0, |s| {
        1.0 - s.weapon.reload_ticks as f32 / s.weapon.reload_total_ticks.max(1) as f32
    });
    let bob = state.map_or(0.0, |s| s.velocity.length().min(12.0) / 12.0) * 0.006;
    for (weapon, mut transform, mut visibility) in &mut guns {
        *visibility = if alive
            && weapon.0 == selected
            && !(selected == WeaponKind::Sniper && mouse.pressed(MouseButton::Right) && !reloading)
        {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        transform.scale = crate::weapon_visual::view_scale(weapon.0);
        let framing_back = (crate::weapon_visual::geometry(weapon.0).scale.z - 1.0).max(0.0) * 0.12;
        transform.translation = Vec3::new(
            tuning.weapon_x,
            tuning.weapon_y - switch * 0.24 - *cooling * 0.04,
            tuning.weapon_z - framing_back,
        ) + Vec3::new(
            (time.elapsed_secs() * 8.0).sin() * bob,
            tuning.recoil_up * kick + (time.elapsed_secs() * 16.0).sin() * bob,
            tuning.recoil_back * kick,
        );
        transform.rotation = Quat::from_euler(
            EulerRot::YXZ,
            tuning.weapon_yaw,
            tuning.weapon_pitch + tuning.recoil_pitch * kick - switch * 0.45 - *cooling * 0.16,
            tuning.recoil_roll * kick + *cooling * 0.1,
        );
        if reloading {
            let (offset, rotation) = crate::reload_animation::pose(selected, reload_progress);
            transform.translation += offset;
            transform.rotation *= rotation;
        }
        if selected == WeaponKind::Karambit {
            // Tilt the horizontal claw toward the camera to show its profile,
            // instead of foreshortening it into a straight blade.
            transform.rotation *= Quat::from_euler(EulerRot::YXZ, 0.45, 0.0, 0.65);
            let swing = (feedback.recoil * std::f32::consts::PI).sin();
            transform.translation += Vec3::new(-0.16, 0.045, -0.13) * swing;
            transform.rotation *=
                Quat::from_euler(EulerRot::YXZ, -swing * 0.65, swing * 0.2, swing * 1.35);
        }
    }
    for (part, mut transform, material, mut visibility) in &mut parts {
        *transform = part.rest;
        if crate::reload_animation::is_round(&part.name) {
            *visibility = Visibility::Hidden;
        }
        if part.kind != selected {
            continue;
        }
        match part.name.as_str() {
            "MovingBolt" => transform.translation.z += feedback.recoil * 0.06,
            "MovingPump" => {
                transform.translation.z += (feedback.recoil * std::f32::consts::PI).sin() * 0.09
            }
            "ChargeCore" => {
                let charge = if reloading {
                    reload_progress
                } else {
                    state.map_or(0.0, |s| {
                        if part.kind == WeaponKind::Arc {
                            (0.35 + feedback.recoil * 0.65).min(1.0)
                        } else {
                            (s.weapon.charge_ticks as f32
                                / (s.tuning.laser_charge * boltrunner_shared::TICK_HZ as f32)
                                    .max(1.0))
                            .min(1.0)
                        }
                    })
                };
                transform.scale *=
                    0.7 + charge * 0.3 + (time.elapsed_secs() * 35.0).sin() * charge * 0.08;
                transform.rotation = Quat::from_rotation_z(time.elapsed_secs() * charge * 4.0);
                if let Some(handle) = material
                    && let Some(material) = materials.get_mut(&handle.0)
                {
                    let [r, g, b] = part.kind.color();
                    let power = 0.15 + charge * (2.0 + (time.elapsed_secs() * 25.0).sin() * 0.5);
                    material.emissive = LinearRgba::new(r * power, g * power, b * power, 1.0);
                }
            }
            "HeatStrip" => {
                let heat = state.map_or(0.0, |s| s.weapon.heat);
                transform.scale.y *= 1.0 + heat * 0.5;
                if let Some(handle) = material
                    && let Some(material) = materials.get_mut(&handle.0)
                {
                    let pulse = 1.0 + *cooling * (time.elapsed_secs() * 12.0).sin() * 0.4;
                    material.emissive = LinearRgba::new(
                        (0.2 + heat * 4.0) * pulse,
                        (0.9 - heat * 0.6) * pulse,
                        0.05,
                        1.0,
                    );
                }
            }
            "CoolingVentLeft" | "CoolingVentRight" => {
                let side = if part.name == "CoolingVentLeft" {
                    -1.0
                } else {
                    1.0
                };
                transform.translation.x += side * *cooling * 0.035;
                transform.rotation =
                    Quat::from_rotation_z(side * *cooling * 0.65) * part.rest.rotation;
            }
            _ => {}
        }
        let base = *transform;
        if let Some(visible) = crate::reload_animation::animate(
            part.kind,
            &part.name,
            base,
            &mut transform,
            reloading.then_some(reload_progress),
        ) {
            *visibility = if visible {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
    }
    for (mut flash, mut transform) in &mut flashes {
        transform.scale = Vec3::new(0.03, 0.03, FLASH_HALF_LENGTH) * tuning.muzzle_flash_size;
        transform.translation.z =
            -(MUZZLE_CLEARANCE + FLASH_HALF_LENGTH * tuning.muzzle_flash_size);
        *flash = if alive
            && !selected.is_stream()
            && selected != WeaponKind::Drone
            && selected != WeaponKind::Karambit
            && feedback.flash > 0.
        {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    feedback.recoil *= (-tuning.recoil_return_speed * time.delta_secs()).exp();
    feedback.flash = (feedback.flash - time.delta_secs()).max(0.);
}

#[derive(Component)]
enum SplashElement {
    Core,
    Halo,
    Ray(Vec3),
}

fn spawn_impact(
    commands: &mut Commands,
    assets: &WeaponAssets,
    kind: WeaponKind,
    position: Vec3,
    normal: Vec3,
    tuning: boltrunner_shared::tuning::PresentationTuning,
) {
    let rotation = Quat::from_rotation_arc(Vec3::Z, normal);
    let (halo, core, rays) = if kind == WeaponKind::HomingPlasma {
        (
            &assets.seeker_glow,
            &assets.seeker_core,
            &assets.colors[kind.index()],
        )
    } else {
        (&assets.glow, &assets.core, &assets.cyan)
    };
    commands
        .spawn((
            ImpactFlash { age: 0. },
            Transform::from_translation(position + normal * 0.06),
            Visibility::Inherited,
        ))
        .with_children(|parent| {
            for (element, material, size) in [
                (SplashElement::Halo, halo, 0.32),
                (SplashElement::Core, core, 0.18),
            ] {
                parent.spawn((
                    Mesh3d(assets.spark.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::from_scale(Vec3::splat(size * tuning.impact_size)),
                    element,
                    NotShadowCaster,
                ));
            }
            for i in 0..tuning.impact_rays {
                let angle = i as f32 * std::f32::consts::TAU / tuning.impact_rays as f32;
                let direction = rotation * Vec3::new(angle.cos(), angle.sin(), 0.45).normalize();
                parent.spawn((
                    Mesh3d(assets.beam.clone()),
                    MeshMaterial3d(rays.clone()),
                    Transform::default(),
                    SplashElement::Ray(direction),
                    NotShadowCaster,
                ));
            }
        });
}

fn fade_impacts(
    settings: Res<boltrunner_client::tuning::TuningClient>,
    mut commands: Commands,
    time: Res<Time>,
    mut impacts: Query<(Entity, &mut ImpactFlash, &Children)>,
    mut elements: Query<(&SplashElement, &mut Transform)>,
) {
    for (entity, mut flash, children) in &mut impacts {
        flash.age += time.delta_secs();
        if flash.age >= settings.values().presentation.impact_time {
            commands.entity(entity).despawn();
            continue;
        }
        let progress = flash.age / settings.values().presentation.impact_time;
        let fade = 1. - progress;
        for child in children {
            let Ok((element, mut transform)) = elements.get_mut(*child) else {
                continue;
            };
            match element {
                SplashElement::Core => transform.scale = Vec3::splat(0.18 * fade * fade),
                SplashElement::Halo => {
                    transform.scale = Vec3::splat((0.32 + progress * 0.6) * fade.sqrt())
                }
                SplashElement::Ray(direction) => {
                    *transform = Transform::from_translation(*direction * progress * 1.15)
                        .looking_to(*direction, Vec3::Y)
                        .with_scale(Vec3::new(0.055 * fade, 0.055 * fade, 0.28 * fade));
                }
            }
            transform.scale *= settings.values().presentation.impact_size;
            transform.translation *= settings.values().presentation.impact_size;
        }
    }
}
