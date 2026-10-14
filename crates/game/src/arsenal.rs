use bevy::{light::NotShadowCaster, prelude::*, scene::SceneInstanceReady};
use boltrunner_shared::{
    PlayerId, PlayerState,
    arsenal::WeaponKind,
    level,
    weapon::{Projectile, ShotImpact},
};
use lightyear::prelude::{Interpolated, Predicted};

pub struct ArsenalVisualsPlugin;
impl Plugin for ArsenalVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DroneBlastView>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                    attach_effects,
                    streams,
                    drones,
                    animate_effects,
                    pellet_impacts,
                    rotors,
                    scope,
                )
                    .chain()
                    .after(crate::weapon::WeaponVisuals),
            );
    }
}
#[derive(Resource, Default)]
pub(crate) struct DroneBlastView {
    pub center: Vec3,
    pub remaining: f32,
    pub duration: f32,
}
#[derive(Resource)]
pub(crate) struct VfxAssets {
    pub texture: Handle<Image>,
    pub flame_texture: Handle<Image>,
    frames: Vec<Handle<Mesh>>,
    fire: Handle<StandardMaterial>,
    flame: Handle<StandardMaterial>,
    plasma: Handle<StandardMaterial>,
    scar: Handle<StandardMaterial>,
    laser_core: Handle<StandardMaterial>,
    jet_core: Handle<StandardMaterial>,
    cone: Handle<Mesh>,
    disc: Handle<Mesh>,
    beam: Handle<Mesh>,
    ring: Handle<Mesh>,
    colors: Vec<Handle<StandardMaterial>>,
}
#[derive(Component)]
struct Attached;
#[derive(Component)]
struct StreamVisual {
    player: Entity,
    kind: WeaponKind,
    strength: f32,
    reach: f32,
}
#[derive(Component)]
struct Puff(usize);
#[derive(Component)]
struct BeamLayer {
    width: f32,
    segment: usize,
}
#[derive(Component)]
struct FlameCore;
#[derive(Component)]
struct DroneVisual(Entity);
#[derive(Component)]
struct Rotor;
#[derive(Component)]
struct Scope;
#[derive(Component)]
struct Explosion {
    radius: f32,
    age: f32,
    kind: WeaponKind,
}
#[derive(Component)]
struct BlastPuff {
    direction: Vec3,
    phase: f32,
}
#[derive(Component)]
struct Shockwave(u8);
#[derive(Component)]
struct FlashCore;
#[derive(Component)]
struct BlastShard(Vec3);
#[derive(Component)]
struct RayFlash {
    origin: Vec3,
    end: Vec3,
    radius: f32,
    kind: WeaponKind,
    age: f32,
}
#[derive(Component)]
struct PelletImpact {
    age: f32,
    normal: Vec3,
    mark: bool,
}
#[derive(Component)]
struct PelletSpark(Vec3);
#[derive(Component)]
struct PelletMark;

fn setup(
    mut commands: Commands,
    server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut frames = Vec::new();
    for frame in 0..16 {
        let mut mesh = Mesh::from(Rectangle::new(1.0, 1.0));
        let column = (frame % 4) as f32;
        let row = (frame / 4) as f32;
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_UV_0,
            vec![
                [(column + 0.002) / 4.0, (row + 0.002) / 4.0],
                [(column + 0.998) / 4.0, (row + 0.002) / 4.0],
                [(column + 0.998) / 4.0, (row + 0.998) / 4.0],
                [(column + 0.002) / 4.0, (row + 0.998) / 4.0],
            ],
        );
        frames.push(meshes.add(mesh));
    }
    let texture = server.load("weapons/arsenal/built/fire.png");
    let fire = materials.add(StandardMaterial {
        base_color_texture: Some(texture.clone()),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        ..default()
    });
    let flame_texture = server.load("weapons/arsenal/built/combat-vfx-v3.png");
    let flame = materials.add(StandardMaterial {
        base_color_texture: Some(flame_texture.clone()),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        ..default()
    });
    let scar = materials.add(StandardMaterial {
        base_color: Color::srgba(0.025, 0.018, 0.014, 0.85),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let plasma = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.28, 0.85, 1.0),
        base_color_texture: Some(flame_texture.clone()),
        unlit: true,
        alpha_mode: AlphaMode::Add,
        cull_mode: None,
        ..default()
    });
    let laser_core = materials.add(StandardMaterial {
        base_color: Color::srgba(0.95, 0.8, 1.0, 0.9),
        unlit: true,
        alpha_mode: AlphaMode::Add,
        ..default()
    });
    let jet_core = materials.add(StandardMaterial {
        base_color: Color::srgba(0.3, 0.65, 1.0, 0.65),
        unlit: true,
        alpha_mode: AlphaMode::Add,
        ..default()
    });
    let colors = WeaponKind::ALL
        .into_iter()
        .map(|kind| {
            let [r, g, b] = kind.color();
            materials.add(StandardMaterial {
                base_color: Color::srgba(r, g, b, 0.7),
                emissive: LinearRgba::new(r * 3.0, g * 3.0, b * 3.0, 1.0),
                unlit: true,
                alpha_mode: AlphaMode::Add,
                ..default()
            })
        })
        .collect();
    commands.insert_resource(VfxAssets {
        texture,
        flame_texture,
        frames,
        fire,
        flame,
        plasma,
        scar,
        laser_core,
        jet_core,
        cone: meshes.add(
            Mesh::from(Cone::new(1.0, 1.0))
                .rotated_by(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        ),
        disc: meshes.add(Circle::new(1.0)),
        beam: meshes.add(
            Mesh::from(Cylinder::new(0.5, 1.0))
                .rotated_by(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        ),
        ring: meshes.add(Annulus::new(0.96, 1.0)),
        colors,
    });
    commands
        .spawn((
            Scope,
            GlobalZIndex(5),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                display: Display::None,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: px(520),
                    height: px(520),
                    max_width: percent(90),
                    max_height: percent(90),
                    border: UiRect::all(px(10)),
                    border_radius: BorderRadius::all(percent(50)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BorderColor::all(Color::srgba(0.015, 0.02, 0.03, 0.9)),
            ))
            .with_children(|ring| {
                for (width, height) in [(percent(100), px(1)), (px(1), percent(100))] {
                    ring.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            width,
                            height,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.12, 0.9, 0.7, 0.7)),
                    ));
                }
            });
        });
}

fn scope(
    look: Res<crate::view::Look>,
    mouse: Res<ButtonInput<MouseButton>>,
    players: Query<&PlayerState, With<Predicted>>,
    mut root: Single<&mut Node, With<Scope>>,
) {
    root.display = if look.locked
        && mouse.pressed(MouseButton::Right)
        && players.iter().any(|s| {
            s.death.is_none()
                && s.weapon.selected == WeaponKind::Sniper
                && s.weapon.reload_ticks == 0
                && s.drone.is_none_or(|d| !d.controlling)
        }) {
        Display::Flex
    } else {
        Display::None
    };
}

pub(crate) fn spawn_explosion(commands: &mut Commands, impact: &ShotImpact) {
    commands.spawn((
        Explosion {
            radius: impact.explosion_radius,
            age: 0.0,
            kind: impact.kind,
        },
        Transform::from_translation(impact.position),
        Visibility::Inherited,
    ));
}
pub(crate) fn spawn_ray(commands: &mut Commands, impact: &ShotImpact) {
    commands.spawn((
        RayFlash {
            origin: impact.origin,
            end: impact.position,
            radius: impact.beam_radius,
            kind: impact.kind,
            age: 0.0,
        },
        Transform::default(),
        Visibility::Inherited,
    ));
}
pub(crate) fn spawn_pellet_impact(commands: &mut Commands, impact: &ShotImpact) {
    commands.spawn((
        PelletImpact {
            age: 0.0,
            normal: impact.normal,
            mark: impact.victim.is_none()
                && level::world()
                    .sweep_sphere(
                        impact.position + impact.normal * 0.04,
                        -impact.normal * 0.08,
                        0.002,
                    )
                    .is_some(),
        },
        Transform::from_translation(impact.position + impact.normal * 0.008),
        Visibility::Inherited,
    ));
}

fn attach_effects(
    mut commands: Commands,
    assets: Res<VfxAssets>,
    explosions: Query<(Entity, &Explosion), Without<Attached>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    rays: Query<(Entity, &RayFlash), Without<Attached>>,
    pellets: Query<(Entity, &PelletImpact), Without<Attached>>,
) {
    for (entity, pellet) in &pellets {
        commands
            .entity(entity)
            .insert(Attached)
            .with_children(|parent| {
                if pellet.mark {
                    parent.spawn((
                        PelletMark,
                        Mesh3d(assets.disc.clone()),
                        MeshMaterial3d(assets.scar.clone()),
                        Transform::from_rotation(Quat::from_rotation_arc(Vec3::Z, pellet.normal))
                            .with_scale(Vec3::splat(0.035)),
                        NotShadowCaster,
                    ));
                }
                let rotation = Quat::from_rotation_arc(Vec3::Z, pellet.normal);
                for i in 0..3 {
                    let angle = i as f32 * 2.39996 + (entity.to_bits() % 4096) as f32 * 0.73;
                    let velocity = rotation * Vec3::new(angle.cos(), angle.sin(), 0.8) * 1.8;
                    parent.spawn((
                        PelletSpark(velocity),
                        Mesh3d(assets.beam.clone()),
                        MeshMaterial3d(assets.colors[WeaponKind::Shotgun.index()].clone()),
                        Transform::default(),
                        NotShadowCaster,
                    ));
                }
            });
    }
    for (entity, explosion) in &explosions {
        let drone = matches!(explosion.kind, WeaponKind::Drone | WeaponKind::Grenade);
        let count = if drone { 20 } else { 12 };
        commands
            .entity(entity)
            .insert(Attached)
            .with_children(|parent| {
                for i in 0..count {
                    let angle = i as f32 * 2.39996;
                    let y = 1.0 - 2.0 * (i as f32 + 0.5) / count as f32;
                    let radial = (1.0 - y * y).sqrt();
                    parent.spawn((
                        BlastPuff {
                            direction: Vec3::new(angle.cos() * radial, y, angle.sin() * radial),
                            phase: i as f32 / count as f32,
                        },
                        Mesh3d(assets.frames[0].clone()),
                        MeshMaterial3d(if explosion.kind == WeaponKind::Grenade {
                            assets.plasma.clone()
                        } else if drone {
                            assets.flame.clone()
                        } else {
                            assets.fire.clone()
                        }),
                        Transform::default(),
                        NotShadowCaster,
                    ));
                }
                for plane in 0..if drone { 3 } else { 1 } {
                    let material = if drone {
                        let [r, g, b] = explosion.kind.color();
                        materials.add(StandardMaterial {
                            base_color: Color::srgba(r, g, b, 0.8),
                            unlit: true,
                            alpha_mode: AlphaMode::Add,
                            cull_mode: None,
                            ..default()
                        })
                    } else {
                        assets.colors[WeaponKind::Rocket.index()].clone()
                    };
                    parent.spawn((
                        Shockwave(plane),
                        Mesh3d(assets.ring.clone()),
                        MeshMaterial3d(material),
                        Transform::default(),
                        NotShadowCaster,
                    ));
                }
                if drone {
                    let material = materials.add(StandardMaterial {
                        base_color: Color::srgba(1.0, 0.9, 0.65, 0.9),
                        base_color_texture: Some(assets.flame_texture.clone()),
                        unlit: true,
                        alpha_mode: AlphaMode::Add,
                        cull_mode: None,
                        ..default()
                    });
                    parent.spawn((
                        FlashCore,
                        Mesh3d(assets.frames[8].clone()),
                        MeshMaterial3d(material),
                        Transform::default(),
                        NotShadowCaster,
                    ));
                    for i in 0..16 {
                        let angle = i as f32 * 2.39996;
                        let y = 1.0 - 2.0 * (i as f32 + 0.5) / 16.0;
                        let radial = (1.0 - y * y).sqrt();
                        let direction = Vec3::new(angle.cos() * radial, y, angle.sin() * radial);
                        parent.spawn((
                            BlastShard(direction),
                            Mesh3d(assets.beam.clone()),
                            MeshMaterial3d(assets.colors[WeaponKind::Shotgun.index()].clone()),
                            Transform::default(),
                            NotShadowCaster,
                        ));
                    }
                }
            });
    }
    for (entity, ray) in &rays {
        let length = ray.origin.distance(ray.end);
        if ray.kind == WeaponKind::Arc {
            let direction = (ray.end - ray.origin).normalize_or_zero();
            let side = direction.any_orthonormal_vector();
            let up = direction.cross(side);
            let count = (length / 1.2).ceil().clamp(4.0, 18.0) as usize;
            let seed = (entity.to_bits() % 1024) as f32;
            commands
                .entity(entity)
                .insert((Attached, Transform::from_translation(ray.origin)))
                .with_children(|parent| {
                    let mut start = Vec3::ZERO;
                    for i in 1..=count {
                        let phase = i as f32 / count as f32;
                        let jitter = (std::f32::consts::PI * phase).sin() * 0.18;
                        let end = direction * length * phase
                            + side * (seed + i as f32 * 8.17).sin() * jitter
                            + up * (seed + i as f32 * 4.73).cos() * jitter;
                        for (width, material) in [
                            (1.0, &assets.colors[ray.kind.index()]),
                            (0.3, &assets.laser_core),
                        ] {
                            parent.spawn((
                                Mesh3d(assets.beam.clone()),
                                MeshMaterial3d(material.clone()),
                                Transform::from_translation((start + end) * 0.5)
                                    .looking_to(end - start, Vec3::Y)
                                    .with_scale(Vec3::new(
                                        ray.radius * 2.0 * width,
                                        ray.radius * 2.0 * width,
                                        start.distance(end).max(0.001),
                                    )),
                                NotShadowCaster,
                            ));
                        }
                        start = end;
                    }
                });
            continue;
        }
        commands
            .entity(entity)
            .insert((
                Attached,
                Transform::from_translation((ray.origin + ray.end) / 2.0)
                    .looking_to(ray.end - ray.origin, Vec3::Y)
                    .with_scale(Vec3::new(
                        ray.radius * 2.0,
                        ray.radius * 2.0,
                        length.max(0.001),
                    )),
            ))
            .with_child((
                Mesh3d(assets.beam.clone()),
                MeshMaterial3d(assets.colors[ray.kind.index()].clone()),
                Transform::default(),
                NotShadowCaster,
            ));
    }
}

type VisiblePlayers = Or<(With<Predicted>, With<Interpolated>)>;
fn streams(
    world_muzzles: Res<crate::world_weapon::WorldMuzzles>,
    ids: Query<&PlayerId>,
    mut commands: Commands,
    assets: Res<VfxAssets>,
    time: Res<Time>,
    camera: Single<&GlobalTransform, With<crate::view::PlayerCamera>>,
    muzzle: Res<crate::weapon::MuzzleGeometry>,
    players: Query<(Entity, &PlayerState, Has<Predicted>), (With<PlayerId>, VisiblePlayers)>,
    mut visuals: Query<(
        Entity,
        &mut StreamVisual,
        &mut Transform,
        &mut Visibility,
        &Children,
    )>,
    mut puffs: Query<(&Puff, &mut Transform, &mut Mesh3d), Without<StreamVisual>>,
    mut beams: Query<
        (&BeamLayer, &mut Transform, &mut Visibility),
        (Without<StreamVisual>, Without<Puff>),
    >,
    mut cores: Query<
        &mut Transform,
        (
            With<FlameCore>,
            Without<StreamVisual>,
            Without<Puff>,
            Without<BeamLayer>,
        ),
    >,
) {
    for (entity, mut visual, mut transform, mut visibility, children) in &mut visuals {
        let Ok((_, state, local)) = players.get(visual.player) else {
            commands.entity(entity).despawn();
            continue;
        };
        let emitting = state.death.is_none()
            && !state.match_paused
            && state.weapon.selected == visual.kind
            && if visual.kind == WeaponKind::MachineGun {
                state.weapon.overheated
            } else {
                state.weapon.firing
            };
        let target = if emitting { 1.0 } else { 0.0 };
        let step = time.delta_secs() * 8.0;
        visual.strength += (target - visual.strength).clamp(-step, step);
        let active = if visual.kind == WeaponKind::Flame {
            visual.strength > 0.001 && state.death.is_none() && state.weapon.selected == visual.kind
        } else {
            emitting
        };
        *visibility = if active {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if !active {
            continue;
        }
        let origin = state.position + Vec3::Y * state.tuning.eye_height;
        let direction =
            Quat::from_euler(EulerRot::YXZ, state.yaw, state.weapon.aim_pitch, 0.0) * Vec3::NEG_Z;
        let stats = visual.kind.stats(state.tuning);
        let mut bolt = Projectile::from_shot(0, state, &default());
        bolt.origin = origin;
        bolt.position = origin;
        bolt.direction = direction;
        let mut ray_segments = Vec::new();
        let length = if visual.kind == WeaponKind::Laser {
            ray_segments =
                boltrunner_shared::weapon::trace_ray_with(&bolt, stats.range, |ray, delta| {
                    boltrunner_shared::weapon::trace(
                        level::world(),
                        ray,
                        delta,
                        players
                            .iter()
                            .filter(|(other, _, _)| *other != visual.player)
                            .map(|(other, s, _)| (other.to_bits(), s)),
                    )
                })
                .segments;
            ray_segments
                .first()
                .map_or(0.0, |segment| segment.origin.distance(segment.end))
        } else {
            let range = if emitting {
                boltrunner_shared::arsenal::flame_reach(state)
            } else {
                visual.reach
            };
            visual.reach = range;
            level::world()
                .sweep_sphere(origin, direction * range, 0.01)
                .map_or(range, |f| f * range)
        };
        // Gameplay is traced from the eye; presentation starts at the weapon socket.
        let endpoint = origin + direction * length;
        let origin = if local {
            muzzle.0.map_or(origin, |geometry| geometry.0)
        } else {
            ids.get(visual.player)
                .ok()
                .and_then(|id| world_muzzles.0.get(&id.0))
                .copied()
                .unwrap_or(origin + direction * 0.45 - Vec3::Y * 0.18)
        };
        let direction = (endpoint - origin).normalize_or_zero();
        let length = origin.distance(endpoint);
        *transform = Transform::from_translation(origin).looking_to(direction, Vec3::Y);
        if let Some(first) = ray_segments.first_mut() {
            first.origin = origin;
        }
        for child in children {
            if let Ok((puff, mut child_transform, mut mesh)) = puffs.get_mut(*child) {
                if visual.kind == WeaponKind::MachineGun {
                    let phase = (time.elapsed_secs() * 1.4 + puff.0 as f32 / 12.0).fract();
                    child_transform.translation = transform.rotation.inverse()
                        * Vec3::new(
                            (puff.0 as f32 * 2.4).sin() * phase * 0.08,
                            phase * 0.45,
                            0.0,
                        );
                    child_transform.rotation = transform.rotation.inverse() * camera.rotation();
                    child_transform.scale = Vec3::splat((0.025 + phase * 0.12) * (1.0 - phase));
                    mesh.0 = assets.frames[12 + (phase * 3.9) as usize].clone();
                } else {
                    let phase = (time.elapsed_secs() * state.tuning.flame_speed / length.max(0.25)
                        + puff.0 as f32 / 24.0)
                        .fract();
                    let angle = puff.0 as f32 * 2.39996 + time.elapsed_secs() * 4.0;
                    let width = 0.055 + phase * length * stats.spread.to_radians().tan() * 0.65;
                    let spread = phase * length * stats.spread.to_radians().tan() * 0.45;
                    child_transform.translation =
                        Vec3::new(angle.cos() * spread, angle.sin() * spread, -phase * length);
                    let screen_direction = camera.rotation().inverse() * direction;
                    let roll = screen_direction.y.atan2(screen_direction.x);
                    child_transform.rotation = transform.rotation.inverse()
                        * camera.rotation()
                        * Quat::from_rotation_z(roll);
                    let fade = (1.0 - phase).powf(0.35);
                    child_transform.scale = Vec3::splat(width * 2.0 * fade * visual.strength);
                    mesh.0 = assets.frames
                        [((time.elapsed_secs() * 18.0) as usize + puff.0 * 3) % 8]
                        .clone();
                }
            }
            if let Ok(mut core) = cores.get_mut(*child) {
                let core_length = length.min(0.45);
                core.translation = Vec3::new(0.0, 0.0, -core_length * 0.5);
                core.scale = Vec3::new(0.022, 0.022, core_length) * visual.strength;
            }
            if let Ok((layer, mut child_transform, mut visible)) = beams.get_mut(*child) {
                let Some(segment) = ray_segments.get(layer.segment) else {
                    *visible = Visibility::Hidden;
                    continue;
                };
                *visible = Visibility::Inherited;
                let inverse = transform.rotation.inverse();
                let start = inverse * (segment.origin - transform.translation);
                let end = inverse * (segment.end - transform.translation);
                let delta = end - start;
                if delta.length_squared() < 1e-10 {
                    *visible = Visibility::Hidden;
                    continue;
                }
                let diameter = state.tuning.laser_radius * 2.0 * layer.width;
                *child_transform = Transform::from_translation((start + end) * 0.5)
                    .with_rotation(Quat::from_rotation_arc(
                        Vec3::NEG_Z,
                        delta.normalize_or_zero(),
                    ))
                    .with_scale(Vec3::new(diameter, diameter, delta.length().max(0.001)));
            }
        }
    }
    for (player, _, _) in &players {
        for kind in [WeaponKind::Flame, WeaponKind::Laser, WeaponKind::MachineGun] {
            if visuals
                .iter()
                .any(|(_, v, _, _, _)| v.player == player && v.kind == kind)
            {
                continue;
            }
            commands
                .spawn((
                    StreamVisual {
                        player,
                        kind,
                        strength: 0.0,
                        reach: 0.0,
                    },
                    Transform::default(),
                    Visibility::Hidden,
                ))
                .with_children(|parent| {
                    if kind != WeaponKind::Laser {
                        for i in 0..if kind == WeaponKind::Flame { 24 } else { 12 } {
                            parent.spawn((
                                Puff(i),
                                Mesh3d(assets.frames[0].clone()),
                                MeshMaterial3d(if kind == WeaponKind::Flame {
                                    assets.flame.clone()
                                } else {
                                    assets.fire.clone()
                                }),
                                Transform::default(),
                                NotShadowCaster,
                            ));
                        }
                        if kind == WeaponKind::Flame {
                            parent.spawn((
                                FlameCore,
                                Mesh3d(assets.cone.clone()),
                                MeshMaterial3d(assets.jet_core.clone()),
                                Transform::default(),
                                NotShadowCaster,
                            ));
                        }
                    } else {
                        for (segment, width) in (0..boltrunner_shared::weapon::MAX_WARP_SEGMENTS)
                            .flat_map(|segment| [1.0, 0.3].map(|width| (segment, width)))
                        {
                            parent.spawn((
                                BeamLayer { width, segment },
                                Mesh3d(assets.beam.clone()),
                                MeshMaterial3d(if width < 1.0 {
                                    assets.laser_core.clone()
                                } else {
                                    assets.colors[kind.index()].clone()
                                }),
                                Transform::default(),
                                Visibility::Hidden,
                                NotShadowCaster,
                            ));
                        }
                    }
                });
        }
    }
}

fn drones(
    mut commands: Commands,
    assets: Res<crate::weapon::WeaponAssets>,
    players: Query<(Entity, &PlayerState, Has<Predicted>), (With<PlayerId>, VisiblePlayers)>,
    mut visuals: Query<(Entity, &DroneVisual, &mut Transform, &mut Visibility)>,
) {
    for (entity, visual, mut transform, mut visibility) in &mut visuals {
        let Ok((_, state, local)) = players.get(visual.0) else {
            commands.entity(entity).despawn();
            continue;
        };
        let Some(drone) = state.drone else {
            commands.entity(entity).despawn();
            continue;
        };
        transform.translation = drone.position;
        transform.rotation = Quat::from_euler(EulerRot::YXZ, drone.yaw, drone.pitch * 0.2, 0.0);
        transform.scale = Vec3::splat(state.tuning.drone_radius / 0.25);
        *visibility = if local && drone.controlling {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
    for (player, state, _) in &players {
        if state.drone.is_some() && !visuals.iter().any(|(_, v, _, _)| v.0 == player) {
            commands
                .spawn((
                    DroneVisual(player),
                    SceneRoot(assets.drone.clone()),
                    Transform::default(),
                    Visibility::Hidden,
                ))
                .observe(prepare_drone);
        }
    }
}
fn prepare_drone(
    event: On<SceneInstanceReady>,
    children: Query<&Children>,
    names: Query<&Name>,
    mut commands: Commands,
) {
    for entity in children.iter_descendants(event.entity) {
        if names.get(entity).is_ok_and(|name| name.as_str() == "Rotor") {
            commands.entity(entity).insert(Rotor);
        }
    }
}
fn rotors(time: Res<Time>, mut rotors: Query<&mut Transform, With<Rotor>>) {
    for mut transform in &mut rotors {
        transform.rotation = Quat::from_rotation_y(time.elapsed_secs() * 90.0);
    }
}
fn animate_effects(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<VfxAssets>,
    camera: Single<&GlobalTransform, With<crate::view::PlayerCamera>>,
    mut explosions: Query<(Entity, &mut Explosion, &Children)>,
    mut puffs: Query<(&BlastPuff, &mut Transform, &mut Mesh3d)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rings: Query<
        (
            &Shockwave,
            &mut Transform,
            &MeshMaterial3d<StandardMaterial>,
        ),
        (Without<BlastPuff>, Without<BlastShard>, Without<FlashCore>),
    >,
    mut shards: Query<(&BlastShard, &mut Transform), (Without<BlastPuff>, Without<FlashCore>)>,
    mut cores: Query<
        (&mut Transform, &MeshMaterial3d<StandardMaterial>),
        (
            With<FlashCore>,
            Without<BlastPuff>,
            Without<BlastShard>,
            Without<Shockwave>,
        ),
    >,
    mut rays: Query<
        (Entity, &mut RayFlash, &mut Transform),
        (
            Without<BlastPuff>,
            Without<Shockwave>,
            Without<BlastShard>,
            Without<FlashCore>,
        ),
    >,
) {
    for (entity, mut explosion, children) in &mut explosions {
        explosion.age += time.delta_secs();
        let drone = matches!(explosion.kind, WeaponKind::Drone | WeaponKind::Grenade);
        let lifetime = if drone { 1.1 } else { 0.65 };
        let progress = (explosion.age / lifetime).min(1.0);
        if progress >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        for child in children {
            if let Ok((puff, mut transform, mut mesh)) = puffs.get_mut(*child) {
                let expansion = if drone {
                    1.0 - (-8.0 * explosion.age).exp()
                } else {
                    progress
                };
                transform.translation =
                    puff.direction * explosion.radius * expansion * if drone { 0.35 } else { 0.55 }
                        + Vec3::Y * progress * if drone { 0.65 } else { 0.25 };
                transform.rotation = camera.rotation();
                transform.scale = Vec3::splat(
                    explosion.radius
                        * (0.2 + expansion * if drone { 0.7 } else { 0.5 })
                        * (1.0 - progress).sqrt(),
                );
                let frame = if drone {
                    8 + ((progress.powf(0.65) + puff.phase * 0.05).min(0.999) * 8.0) as usize
                } else {
                    ((progress + puff.phase * 0.08).min(0.999) * 16.0) as usize
                };
                mesh.0 = assets.frames[frame].clone();
            }
            if let Ok((wave, mut transform, material)) = rings.get_mut(*child) {
                transform.rotation = match wave.0 {
                    0 => Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
                    1 => camera.rotation(),
                    _ => Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
                };
                if drone {
                    let wave_age = (explosion.age - wave.0 as f32 * 0.025).max(0.0);
                    transform.scale =
                        Vec3::splat(explosion.radius * (1.0 - (-12.0 * wave_age).exp()));
                    if let Some(material) = materials.get_mut(&material.0) {
                        let [r, g, b] = explosion.kind.color();
                        material.base_color =
                            Color::srgba(r, g, b, (1.0 - wave_age / 0.42).max(0.0) * 0.8);
                    }
                } else {
                    transform.scale = Vec3::splat(
                        explosion.radius
                            * (explosion.age / 0.28).min(1.0)
                            * (1.0 - progress).sqrt(),
                    );
                }
            }
            if let Ok((shard, mut transform)) = shards.get_mut(*child) {
                let fade = (1.0 - explosion.age / 0.75).max(0.0);
                let direction = shard.0 - Vec3::Y * explosion.age * 0.65;
                *transform = Transform::from_translation(
                    shard.0 * explosion.radius * (1.0 - (-6.0 * explosion.age).exp())
                        - Vec3::Y * explosion.age.powi(2) * 1.5,
                )
                .looking_to(direction, Vec3::Y)
                .with_scale(Vec3::new(
                    0.012 * fade,
                    0.012 * fade,
                    (0.3 + explosion.radius * 0.08) * fade,
                ));
            }
            if let Ok((mut transform, material)) = cores.get_mut(*child) {
                let fade = (1.0 - explosion.age / 0.16).max(0.0);
                transform.rotation = camera.rotation();
                transform.scale = Vec3::splat(explosion.radius * (0.2 + explosion.age * 2.0));
                if let Some(material) = materials.get_mut(&material.0) {
                    material.base_color = Color::srgba(1.0, 0.9, 0.65, fade * 0.9);
                }
            }
        }
    }
    for (entity, mut ray, mut transform) in &mut rays {
        ray.age += time.delta_secs();
        if ray.age >= 0.12 {
            commands.entity(entity).despawn();
        } else if ray.kind != WeaponKind::Arc {
            let diameter = ray.radius * 2.0 * (1.0 - ray.age / 0.12);
            transform.scale.x = diameter;
            transform.scale.y = diameter;
        }
    }
}

fn pellet_impacts(
    mut commands: Commands,
    time: Res<Time>,
    mut impacts: Query<(Entity, &mut PelletImpact, &Children)>,
    mut sparks: Query<(&PelletSpark, &mut Transform), Without<PelletMark>>,
    mut marks: Query<&mut Transform, (With<PelletMark>, Without<PelletSpark>)>,
) {
    for (entity, mut impact, children) in &mut impacts {
        impact.age += time.delta_secs();
        let lifetime = if impact.mark { 8.0 } else { 0.18 };
        if impact.age >= lifetime {
            commands.entity(entity).despawn();
            continue;
        }
        for child in children {
            if let Ok((spark, mut transform)) = sparks.get_mut(*child) {
                let fade = (1.0 - impact.age / 0.14).max(0.0);
                let velocity = spark.0 - Vec3::Y * impact.age * 4.0;
                *transform = Transform::from_translation(
                    spark.0 * impact.age - Vec3::Y * 2.0 * impact.age.powi(2),
                )
                .looking_to(velocity, Vec3::Y)
                .with_scale(Vec3::new(0.007 * fade, 0.007 * fade, 0.055 * fade));
            }
            if let Ok(mut transform) = marks.get_mut(*child) {
                transform.scale = Vec3::splat(0.035 * (8.0 - impact.age).min(1.0));
            }
        }
    }
}
