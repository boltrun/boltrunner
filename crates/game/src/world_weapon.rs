//! World weapons and their muzzle positions, independent of character meshes.
use bevy::{
    camera::visibility::RenderLayers, light::NotShadowCaster, prelude::*,
    scene::SceneInstanceReady, transform::helper::TransformHelper,
};
use boltrunner_shared::{PlayerId, PlayerInput, PlayerState, equipment::*};
use std::collections::HashMap;

pub struct WorldWeaponsPlugin;
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct WorldWeaponsUpdated;

#[derive(Component, Clone, Copy)]
pub(crate) struct WeaponGrip {
    pub height: f32,
    pub forward: f32,
    pub side: f32,
    pub two_handed: bool,
    pub preview: bool,
    pub simple_arms: bool,
}
impl Default for WeaponGrip {
    fn default() -> Self {
        Self {
            height: 0.70,
            forward: 0.45,
            side: 0.27,
            two_handed: true,
            preview: false,
            simple_arms: true,
        }
    }
}

#[derive(Component, Clone, Copy)]
pub(crate) struct WeaponHandTargets {
    pub right: Vec3,
    pub left: Option<Vec3>,
    pub rotation: Quat,
}

#[derive(Resource, Default)]
pub(crate) struct WorldMuzzles(pub HashMap<u64, Vec3>);
#[derive(Resource, Default)]
struct Models(HashMap<u8, Handle<Scene>>);
#[derive(Component)]
struct Equipped {
    root: Entity,
    kind: u8,
    shot: u16,
    kick: f32,
}
#[derive(Component, Clone, Copy)]
struct WorldWeapon {
    owner: Entity,
    preview: bool,
}
#[derive(Component)]
struct WorldReloadPart {
    owner: Entity,
    kind: boltrunner_shared::arsenal::WeaponKind,
    name: String,
    rest: Transform,
}
#[derive(Component)]
struct Muzzle(Entity);
#[derive(Component)]
struct SimpleArm {
    owner: Entity,
    side: f32,
    segment: u8,
}
#[derive(Component)]
struct SimpleArms;
#[derive(Component)]
struct Flash(Entity);
#[derive(Resource)]
struct FlashAssets {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    limb: Handle<Mesh>,
}

impl Plugin for WorldWeaponsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Models>()
            .init_resource::<WorldMuzzles>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (place, animate_reload_parts, simple_arms, muzzles, flashes)
                    .chain()
                    .in_set(WorldWeaponsUpdated)
                    .after(crate::view::CameraUpdated),
            );
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(FlashAssets {
        mesh: meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap()),
        limb: meshes.add(Capsule3d::new(0.055, 1.0)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.85, 0.98, 1.0),
            emissive: LinearRgba::new(3.0, 4.0, 5.0, 1.0),
            unlit: true,
            ..default()
        }),
    });
}

fn place(
    mut commands: Commands,
    time: Res<Time>,
    server: Res<AssetServer>,
    mut models: ResMut<Models>,
    mut owners: Query<
        (
            Entity,
            &PlayerEquipment,
            Option<&PlayerState>,
            Option<&WeaponGrip>,
            Option<&mut Equipped>,
        ),
        With<GlobalTransform>,
    >,
    mut roots: Query<(&mut Transform, &mut Visibility), With<WorldWeapon>>,
) {
    for (owner, equipment, state, grip, attached) in &mut owners {
        let grip = grip.copied().unwrap_or_default();
        let kind = equipment.weapon.min((WEAPON_MODELS.len() - 1) as u8);
        let factor = state.map_or(1.0, |s| s.tuning.player_height / 1.8);
        let kind_value = boltrunner_shared::arsenal::WeaponKind::ALL[kind as usize];
        let (reload_offset, reload_rotation) =
            state
                .filter(|s| s.weapon.reload_ticks > 0)
                .map_or((Vec3::ZERO, Quat::IDENTITY), |s| {
                    let progress = 1.0
                        - s.weapon.reload_ticks as f32 / s.weapon.reload_total_ticks.max(1) as f32;
                    crate::reload_animation::pose(kind_value, progress)
                });
        let right =
            (Vec3::new(grip.side, grip.height * 1.8, -grip.forward) + reload_offset * 0.4) * factor;
        let pitch = equipment.pitch as f32 / i16::MAX as f32 * PlayerInput::MAX_PITCH;
        let rotation = Quat::from_rotation_x(pitch) * Quat::IDENTITY.slerp(reload_rotation, 0.6);
        let model = crate::weapon_visual::geometry(
            boltrunner_shared::arsenal::WeaponKind::ALL[kind as usize],
        );
        let scale = model.scale * factor;
        let handle = model.grip;
        let support = model.support;
        commands.entity(owner).insert(WeaponHandTargets {
            right,
            left: support
                .filter(|_| grip.two_handed)
                .map(|support| right + rotation * ((support - handle) * scale)),
            rotation,
        });
        let mut kick = 0.0;
        if let Some(mut attached) = attached {
            if attached.kind != kind {
                commands.entity(attached.root).try_despawn();
                commands.entity(owner).remove::<Equipped>();
            } else {
                let shot = state.map_or(0, |s| s.weapon.shot);
                if attached.shot != shot {
                    attached.shot = shot;
                    attached.kick = 1.0;
                }
                attached.kick *= (-18.0 * time.delta_secs()).exp();
                kick = attached.kick;
                if let Ok((mut transform, mut visible)) = roots.get_mut(attached.root) {
                    *transform = Transform::from_translation(
                        right - rotation * (handle * scale) + rotation * Vec3::Z * 0.018 * kick,
                    )
                    .with_rotation(rotation * Quat::from_rotation_x(-0.04 * kick))
                    .with_scale(scale);
                    *visible = if state.is_none_or(|s| s.death.is_none()) {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    };
                }
                continue;
            }
        }
        let scene = models
            .0
            .entry(kind)
            .or_insert_with(|| {
                server.load(GltfAssetLabel::Scene(0).from_asset(WEAPON_MODELS[kind as usize]))
            })
            .clone();
        let root = commands
            .spawn((
                SceneRoot(scene),
                WorldWeapon {
                    owner,
                    preview: grip.preview,
                },
                Transform::from_translation(right - rotation * (handle * scale))
                    .with_rotation(rotation)
                    .with_scale(scale),
                Visibility::Inherited,
            ))
            .observe(ready)
            .id();
        commands.entity(owner).add_child(root).insert(Equipped {
            root,
            kind,
            shot: state.map_or(0, |s| s.weapon.shot),
            kick,
        });
    }
}

fn ready(
    event: On<SceneInstanceReady>,
    mut commands: Commands,
    roots: Query<&WorldWeapon>,
    children: Query<&Children>,
    names: Query<&Name>,
    assets: Res<FlashAssets>,
    transforms: Query<&Transform>,
    equipped: Query<&Equipped>,
) {
    let Ok(weapon) = roots.get(event.entity) else {
        return;
    };
    for entity in children.iter_descendants(event.entity) {
        let layer = if weapon.preview { 2 } else { 0 };
        commands.entity(entity).insert(RenderLayers::layer(layer));
        if let (Ok(name), Ok(rest), Ok(held)) = (
            names.get(entity),
            transforms.get(entity),
            equipped.get(weapon.owner),
        ) {
            let kind = boltrunner_shared::arsenal::WeaponKind::ALL[held.kind as usize];
            if crate::reload_animation::is_part(kind, name.as_str()) {
                commands.entity(entity).insert(WorldReloadPart {
                    owner: weapon.owner,
                    kind,
                    name: name.as_str().into(),
                    rest: *rest,
                });
                if crate::reload_animation::is_round(name.as_str()) {
                    commands.entity(entity).insert(Visibility::Hidden);
                }
            }
        }
        if names
            .get(entity)
            .is_ok_and(|name| name.as_str() == "Muzzle")
        {
            commands
                .entity(entity)
                .insert(Muzzle(weapon.owner))
                .with_child((
                    Mesh3d(assets.mesh.clone()),
                    MeshMaterial3d(assets.material.clone()),
                    Transform::from_xyz(0.0, 0.0, -0.06).with_scale(Vec3::new(0.035, 0.035, 0.08)),
                    Visibility::Hidden,
                    RenderLayers::layer(layer),
                    NotShadowCaster,
                    Flash(weapon.owner),
                ));
        }
    }
}

fn animate_reload_parts(
    players: Query<&PlayerState>,
    mut parts: Query<(&WorldReloadPart, &mut Transform, &mut Visibility)>,
) {
    for (part, mut transform, mut visibility) in &mut parts {
        *transform = part.rest;
        let progress = players
            .get(part.owner)
            .ok()
            .filter(|s| s.weapon.reload_ticks > 0)
            .map(|s| {
                1.0 - s.weapon.reload_ticks as f32 / s.weapon.reload_total_ticks.max(1) as f32
            });
        if let Some(visible) = crate::reload_animation::animate(
            part.kind,
            &part.name,
            part.rest,
            &mut transform,
            progress,
        ) {
            *visibility = if visible {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
    }
}

fn muzzles(
    helper: TransformHelper,
    sockets: Query<(Entity, &Muzzle)>,
    owners: Query<&PlayerId>,
    mut world: ResMut<WorldMuzzles>,
) {
    world.0.clear();
    for (entity, muzzle) in &sockets {
        if let (Ok(id), Ok(transform)) = (
            owners.get(muzzle.0),
            helper.compute_global_transform(entity),
        ) {
            world.0.insert(id.0, transform.translation());
        }
    }
}

fn flashes(
    owners: Query<(&Equipped, Option<&PlayerState>)>,
    mut flashes: Query<(&Flash, &mut Visibility)>,
) {
    for (flash, mut visible) in &mut flashes {
        *visible = if owners
            .get(flash.0)
            .is_ok_and(|(gun, state)| gun.kick > 0.30 && state.is_none_or(|s| s.death.is_none()))
        {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

fn simple_arms(
    mut commands: Commands,
    assets: Res<FlashAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    owners: Query<(
        Entity,
        &WeaponHandTargets,
        Option<&WeaponGrip>,
        Option<&PlayerState>,
        Option<&boltrunner_shared::player_color::PlayerColor>,
        Has<SimpleArms>,
    )>,
    mut limbs: Query<(&SimpleArm, &mut Transform, &mut Visibility)>,
) {
    for (owner, targets, grip, state, color, attached) in &owners {
        let grip = grip.copied().unwrap_or_default();
        let factor = state.map_or(1.0, |s| s.tuning.player_height / 1.8);
        let visible = grip.simple_arms && state.is_none_or(|s| s.death.is_none());
        if !attached {
            let rgb = color.map_or([0.32, 0.68, 0.8], |c| c.rgb());
            let material = materials.add(StandardMaterial {
                base_color: Color::srgb(rgb[0], rgb[1], rgb[2]),
                perceptual_roughness: 1.0,
                ..default()
            });
            commands
                .entity(owner)
                .insert(SimpleArms)
                .with_children(|parent| {
                    for side in [-1.0, 1.0] {
                        for segment in 0..3 {
                            parent.spawn((
                                SimpleArm {
                                    owner,
                                    side,
                                    segment,
                                },
                                Mesh3d(if segment == 2 {
                                    assets.mesh.clone()
                                } else {
                                    assets.limb.clone()
                                }),
                                MeshMaterial3d(material.clone()),
                                RenderLayers::layer(if grip.preview { 2 } else { 0 }),
                                NotShadowCaster,
                                Transform::default(),
                                Visibility::Hidden,
                            ));
                        }
                    }
                });
        }
        for (limb, mut transform, mut visibility) in &mut limbs {
            if limb.owner != owner {
                continue;
            }
            *visibility = if visible {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            if !visible {
                continue;
            }
            let shoulder = Vec3::new(limb.side * 0.42, 1.23, -0.05) * factor;
            let hand = if limb.side > 0.0 {
                targets.right
            } else {
                targets
                    .left
                    .unwrap_or(Vec3::new(-0.32, 0.75, -0.1) * factor)
            };
            let elbow = (shoulder + hand) * 0.5 + Vec3::new(limb.side * 0.11, -0.16, 0.08) * factor;
            if limb.segment == 2 {
                *transform = Transform::from_translation(hand)
                    .with_rotation(targets.rotation)
                    .with_scale(Vec3::splat(0.064 * factor));
            } else {
                let (a, b) = if limb.segment == 0 {
                    (shoulder, elbow)
                } else {
                    (elbow, hand)
                };
                *transform = Transform::from_translation((a + b) * 0.5)
                    .with_rotation(Quat::from_rotation_arc(
                        Vec3::Y,
                        (b - a).normalize_or_zero(),
                    ))
                    .with_scale(Vec3::new(factor, a.distance(b) / 1.11, factor));
            }
        }
    }
}
