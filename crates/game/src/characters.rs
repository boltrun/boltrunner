//! Cosmetic scenes are separate from the replicated movement capsule.
use bevy::transform::{TransformSystems, helper::TransformHelper};
use bevy::world_serialization::{WorldAsset, WorldAssetRoot};
use bevy::{
    app::AnimationSystems, asset::LoadState, camera::visibility::RenderLayers, gltf::Gltf,
    prelude::*, world_serialization::WorldInstanceReady,
};
use boltrunner_shared::{PlayerState, characters::*};
use std::{collections::HashMap, time::Duration};

#[derive(Resource, Default, Deref, DerefMut)]
pub struct CurrentCatalog(pub CharacterCatalog);

pub struct CharactersPlugin;
impl Plugin for CharactersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CurrentCatalog>()
            .init_resource::<CharacterModels>()
            .add_systems(
                Update,
                (catalog, player_bodies, load_models, instances, animate)
                    .chain()
                    .after(crate::view::CameraUpdated),
            )
            .add_systems(
                PostUpdate,
                (pose_arms, pose_cloth)
                    .chain()
                    .after(AnimationSystems)
                    .before(TransformSystems::Propagate),
            );
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Pose {
    #[default]
    Idle,
    Walk,
    Run,
    Jump,
    Dash,
    Death,
}
impl Pose {
    fn clip(self, entry: &CharacterDefinition) -> Option<&str> {
        let a = &entry.animations;
        match self {
            Self::Idle => a.idle.as_deref(),
            Self::Walk => a.walk.as_deref(),
            Self::Run => a.run.as_deref().or(a.walk.as_deref()),
            Self::Jump => a.jump.as_deref().or(a.idle.as_deref()),
            Self::Dash => a.dash.as_deref().or(a.run.as_deref()),
            Self::Death => a.death.as_deref().or(a.idle.as_deref()),
        }
    }
}

#[derive(Component, Clone)]
pub struct CharacterBody {
    pub id: String,
    pub height: f32,
    pub pose: Pose,
    pub speed: f32,
    pub preview: bool,
}
#[derive(Component)]
pub(crate) struct FallbackCharacter;
#[derive(Component)]
struct AttachedCharacter {
    definition: CharacterDefinition,
    root: Entity,
    ready: bool,
}
#[derive(Component)]
struct ModelInstance {
    owner: Entity,
    definition: CharacterDefinition,
    graph: Handle<AnimationGraph>,
    clips: HashMap<Pose, AnimationNodeIndex>,
    preview: bool,
}
#[derive(Component)]
struct AnimatedCharacter {
    owner: Entity,
    clips: HashMap<Pose, AnimationNodeIndex>,
    current: Option<Pose>,
}
#[derive(Component)]
struct CharacterBone {
    owner: Entity,
    rest: Transform,
    role: usize,
}

struct ModelAsset {
    definition: CharacterDefinition,
    gltf: Handle<Gltf>,
    scene: Option<Handle<WorldAsset>>,
    graph: Handle<AnimationGraph>,
    clips: HashMap<Pose, AnimationNodeIndex>,
    error: Option<String>,
}
#[derive(Resource, Default)]
pub struct CharacterModels {
    entries: HashMap<String, ModelAsset>,
}
impl CharacterModels {
    pub fn status(&self, entry: &CharacterDefinition) -> &str {
        match self.entries.get(&entry.id) {
            Some(asset) if asset.error.is_some() => "Could not load model. You can still play.",
            Some(asset) if asset.scene.is_some() => "",
            _ => "Loading character…",
        }
    }
}

fn catalog(mut current: ResMut<CurrentCatalog>, snapshots: Query<&CharacterCatalog>) {
    if let Ok(snapshot) = snapshots.single() {
        if current.0 != *snapshot {
            current.0 = snapshot.clone();
        }
    } else if !current.characters.is_empty() {
        *current = default();
    }
}

fn player_bodies(
    mut commands: Commands,
    catalog: Res<CurrentCatalog>,
    players: Query<(Entity, &PlayerCharacter, &PlayerState), With<crate::view::PlayerVisual>>,
) {
    for (entity, selected, state) in &players {
        let speed = state.velocity.length();
        let pose = if state.death.is_some() {
            Pose::Death
        } else if state.dash.remaining_ticks > 0 {
            Pose::Dash
        } else if !state.grounded {
            Pose::Jump
        } else if speed > 7.0 {
            Pose::Run
        } else if speed > 0.25 {
            Pose::Walk
        } else {
            Pose::Idle
        };
        commands.entity(entity).insert(CharacterBody {
            id: selected.0.clone(),
            height: state.tuning.player_height,
            pose,
            speed,
            preview: false,
        });
        let height = catalog
            .find(&selected.0)
            .and_then(|entry| entry.arms.as_ref())
            .map_or(0.70, |arms| arms.height);
        commands
            .entity(entity)
            .insert(crate::world_weapon::WeaponGrip {
                height,
                two_handed: catalog
                    .find(&selected.0)
                    .and_then(|e| e.arms.as_ref())
                    .is_none_or(|a| a.two_handed),
                forward: catalog
                    .find(&selected.0)
                    .and_then(|e| e.arms.as_ref())
                    .map_or(0.45, |a| a.forward),
                side: catalog
                    .find(&selected.0)
                    .and_then(|e| e.arms.as_ref())
                    .map_or(0.27, |a| a.side),
                preview: false,
                simple_arms: catalog.find(&selected.0).is_none(),
            });
    }
}

fn load_models(
    bodies: Query<&CharacterBody>,
    catalog: Res<CurrentCatalog>,
    server: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut models: ResMut<CharacterModels>,
) {
    for body in &bodies {
        let Some(entry) = catalog.find(&body.id) else {
            continue;
        };
        if models
            .entries
            .get(&entry.id)
            .is_some_and(|asset| asset.definition != *entry)
        {
            models.entries.remove(&entry.id);
        }
        let asset = models
            .entries
            .entry(entry.id.clone())
            .or_insert_with(|| ModelAsset {
                definition: entry.clone(),
                // Loading by asset type also accepts a VRM (binary glTF) container.
                gltf: server.load::<Gltf>(entry.model.clone()),
                scene: None,
                graph: default(),
                clips: default(),
                error: None,
            });
        if asset.scene.is_some() || asset.error.is_some() {
            continue;
        }
        if let Some(gltf) = gltfs.get(&asset.gltf) {
            let Some(scene) = gltf.default_scene.as_ref().or(gltf.scenes.first()) else {
                asset.error = Some("The file contains no scene.".into());
                continue;
            };
            let poses = [
                Pose::Idle,
                Pose::Walk,
                Pose::Run,
                Pose::Jump,
                Pose::Dash,
                Pose::Death,
            ];
            let clips: Vec<_> = poses
                .into_iter()
                .filter_map(|pose| {
                    let clip = gltf.named_animations.get(pose.clip(entry)?).cloned()?;
                    Some((pose, clip))
                })
                .collect();
            let (graph, indices) =
                AnimationGraph::from_clips(clips.iter().map(|(_, clip)| clip.clone()));
            asset.graph = graphs.add(graph);
            asset.clips = clips
                .into_iter()
                .zip(indices)
                .map(|((pose, _), index)| (pose, index))
                .collect();
            asset.scene = Some(scene.clone());
        } else if let Some(LoadState::Failed(error)) = server.get_load_state(asset.gltf.id()) {
            warn!("Character {}: {error}", entry.id);
            asset.error = Some(error.to_string());
        }
    }
}

fn instances(
    mut commands: Commands,
    bodies: Query<(Entity, &CharacterBody, Option<&AttachedCharacter>)>,
    catalog: Res<CurrentCatalog>,
    models: Res<CharacterModels>,
    mut roots: Query<
        (&mut Transform, &mut Visibility),
        (Without<CharacterBody>, Without<FallbackCharacter>),
    >,
    children: Query<&Children>,
    mut fallback: Query<&mut Visibility, With<FallbackCharacter>>,
) {
    for (owner, body, attached) in &bodies {
        let definition = catalog.find(&body.id);
        let same = attached.is_some_and(|attached| definition == Some(&attached.definition));
        let ready = same && attached.is_some_and(|attached| attached.ready);
        if let Ok(children) = children.get(owner) {
            for child in children {
                if let Ok(mut visibility) = fallback.get_mut(*child) {
                    *visibility = if ready {
                        Visibility::Hidden
                    } else {
                        Visibility::Inherited
                    };
                }
            }
        }
        if let Some(attached) = attached {
            if !same {
                commands.entity(attached.root).despawn();
                commands.entity(owner).remove::<AttachedCharacter>();
            } else {
                if let Ok((mut transform, mut visibility)) = roots.get_mut(attached.root) {
                    *transform = model_transform(&attached.definition, body.height);
                    *visibility = if attached.ready {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    };
                }
                continue;
            }
        }
        let Some(entry) = definition else { continue };
        let Some(asset) = models.entries.get(&entry.id) else {
            continue;
        };
        let Some(scene) = &asset.scene else { continue };
        let root = commands
            .spawn((
                WorldAssetRoot(scene.clone()),
                Visibility::Hidden,
                model_transform(entry, body.height),
                ModelInstance {
                    owner,
                    definition: entry.clone(),
                    graph: asset.graph.clone(),
                    clips: asset.clips.clone(),
                    preview: body.preview,
                },
            ))
            .observe(scene_ready)
            .id();
        commands
            .entity(owner)
            .add_child(root)
            .insert(AttachedCharacter {
                definition: entry.clone(),
                root,
                ready: false,
            });
    }
}

fn model_transform(entry: &CharacterDefinition, height: f32) -> Transform {
    let factor = height / 1.8;
    Transform::from_translation(entry.offset * factor)
        .with_scale(Vec3::splat(entry.scale * factor))
        .with_rotation(Quat::from_rotation_y(entry.yaw))
}

fn scene_ready(
    event: On<WorldInstanceReady>,
    mut commands: Commands,
    roots: Query<&ModelInstance>,
    children: Query<&Children>,
    mut attached: Query<&mut AttachedCharacter>,
    mut players: Query<&mut AnimationPlayer>,
    names: Query<(&Name, &Transform)>,
) {
    let Ok(instance) = roots.get(event.entity) else {
        return;
    };
    if let Ok(mut attached) = attached.get_mut(instance.owner) {
        if attached.root != event.entity {
            return;
        }
        attached.ready = true;
    }
    for entity in children.iter_descendants(event.entity) {
        if instance.preview {
            commands.entity(entity).insert(RenderLayers::layer(2));
        }
        if let Ok(mut player) = players.get_mut(entity) {
            if let Some(index) = instance.clips.get(&Pose::Idle) {
                player.play(*index).repeat();
            }
            commands.entity(entity).insert((
                AnimationGraphHandle(instance.graph.clone()),
                AnimationTransitions::new(),
                AnimatedCharacter {
                    owner: instance.owner,
                    clips: instance.clips.clone(),
                    current: None,
                },
            ));
        }
        if let (Some(rig), Ok((name, rest))) = (&instance.definition.rig, names.get(entity)) {
            let bones = [
                &rig.left_arm,
                &rig.right_arm,
                &rig.left_leg,
                &rig.right_leg,
                &rig.left_knee,
                &rig.right_knee,
                &rig.chest,
            ];
            if let Some(role) = bones.iter().position(|bone| bone.as_str() == name.as_str()) {
                commands.entity(entity).insert(CharacterBone {
                    owner: instance.owner,
                    rest: *rest,
                    role,
                });
            }
        }
        if let Some(arms) = &instance.definition.arms {
            if let Ok((name, _)) = names.get(entity) {
                if arms.hanging.iter().any(|root| root == name.as_str()) {
                    if let Ok(descendants) = children.get(entity) {
                        if let Some(child) = descendants.first() {
                            commands.entity(entity).insert(ClothChain {
                                child: *child,
                                owner: instance.owner,
                            });
                        }
                    }
                }
                let side = if name.as_str() == arms.right_upper {
                    Some(true)
                } else if name.as_str() == arms.left_upper {
                    Some(false)
                } else {
                    None
                };
                if let Some(right) = side {
                    let (lower, hand, rotation) = if right {
                        (&arms.right_lower, &arms.right_hand, arms.right_rotation)
                    } else {
                        (&arms.left_lower, &arms.left_hand, arms.left_rotation)
                    };
                    let find = |target: &str| {
                        children.iter_descendants(event.entity).find(|node| {
                            names
                                .get(*node)
                                .is_ok_and(|(name, _)| name.as_str() == target)
                        })
                    };
                    if let (Some(lower), Some(hand)) = (find(lower), find(hand)) {
                        commands.entity(entity).insert(ArmChain {
                            owner: instance.owner,
                            lower,
                            hand,
                            right,
                            rotation,
                        });
                    }
                }
            }
        }
    }
}

#[derive(Component)]
struct ArmChain {
    owner: Entity,
    lower: Entity,
    hand: Entity,
    right: bool,
    rotation: Option<Vec3>,
}

fn pose_arms(
    mut transforms: ParamSet<(TransformHelper, Query<&mut Transform>)>,
    chains: Query<(Entity, &ArmChain)>,
    parents: Query<&ChildOf>,
    owners: Query<(&CharacterBody, &crate::world_weapon::WeaponHandTargets)>,
) {
    for (upper, chain) in &chains {
        let Ok((body, targets)) = owners.get(chain.owner) else {
            continue;
        };
        if body.pose == Pose::Death {
            continue;
        }
        let globals = {
            let helper = transforms.p0();
            let Ok(parent) = parents.get(upper) else {
                continue;
            };
            let values = [chain.owner, parent.parent(), upper, chain.lower, chain.hand]
                .map(|entity| helper.compute_global_transform(entity));
            let [Ok(owner), Ok(parent), Ok(shoulder), Ok(elbow), Ok(hand)] = values else {
                continue;
            };
            (owner, parent, shoulder, elbow, hand)
        };
        let (owner, parent, shoulder, elbow, hand) = globals;
        let local_target = if chain.right {
            Some(targets.right)
        } else {
            targets.left
        };
        let Some(local_target) = local_target else {
            continue;
        };
        let target = owner.transform_point(local_target);
        let start = shoulder.translation();
        let upper_length = start.distance(elbow.translation());
        let lower_length = elbow.translation().distance(hand.translation());
        if upper_length < 0.015 || lower_length < 0.015 {
            continue;
        }
        let direction = (target - start).normalize_or_zero();
        let distance = start.distance(target).clamp(
            (upper_length - lower_length).abs() + 0.001,
            (upper_length + lower_length) * 0.998,
        );
        let along = (upper_length * upper_length + distance * distance
            - lower_length * lower_length)
            / (2.0 * distance);
        let outward =
            owner.rotation() * Vec3::new(if chain.right { 0.45 } else { -0.45 }, -1.0, 0.1);
        let bend = (outward - direction * outward.dot(direction)).normalize_or_zero();
        let elbow_target = start
            + direction * along
            + bend
                * (upper_length * upper_length - along * along)
                    .max(0.0)
                    .sqrt();
        let delta = Quat::from_rotation_arc(
            (elbow.translation() - start).normalize(),
            (elbow_target - start).normalize(),
        );
        if let Ok(mut transform) = transforms.p1().get_mut(upper) {
            transform.rotation =
                (parent.rotation().inverse() * delta * shoulder.rotation()).normalize();
        }
        let (elbow, hand, parent) = {
            let helper = transforms.p0();
            let (Ok(elbow), Ok(hand), Ok(parent)) = (
                helper.compute_global_transform(chain.lower),
                helper.compute_global_transform(chain.hand),
                helper.compute_global_transform(upper),
            ) else {
                continue;
            };
            (elbow, hand, parent)
        };
        let direction = (target - elbow.translation()).normalize_or_zero();
        let delta = Quat::from_rotation_arc(
            (hand.translation() - elbow.translation()).normalize(),
            direction,
        );
        if let Ok(mut transform) = transforms.p1().get_mut(chain.lower) {
            transform.rotation =
                (parent.rotation().inverse() * delta * elbow.rotation()).normalize();
        }
        if let Some(offset) = chain.rotation {
            let parent = {
                let helper = transforms.p0();
                let Ok(parent) = helper.compute_global_transform(chain.lower) else {
                    continue;
                };
                parent
            };
            if let Ok(mut transform) = transforms.p1().get_mut(chain.hand) {
                transform.rotation = (parent.rotation().inverse()
                    * owner.rotation()
                    * targets.rotation
                    * Quat::from_euler(EulerRot::XYZ, offset.x, offset.y, offset.z))
                .normalize();
            }
        }
    }
}

fn animate(
    time: Res<Time>,
    bodies: Query<&CharacterBody>,
    catalog: Res<CurrentCatalog>,
    mut players: Query<(
        &mut AnimationPlayer,
        &mut AnimationTransitions,
        &mut AnimatedCharacter,
    )>,
    mut bones: Query<(&CharacterBone, &mut Transform)>,
) {
    for (mut player, mut transitions, mut animation) in &mut players {
        let Ok(body) = bodies.get(animation.owner) else {
            continue;
        };
        if animation.current != Some(body.pose) {
            if let Some(index) = animation
                .clips
                .get(&body.pose)
                .or(animation.clips.get(&Pose::Idle))
            {
                let active = transitions.play(&mut player, *index, Duration::from_millis(120));
                active.set_speed(1.0);
                if body.pose != Pose::Death {
                    active.repeat();
                }
            }
            animation.current = Some(body.pose);
        }
        if matches!(body.pose, Pose::Walk | Pose::Run) {
            if let Some(index) = animation.clips.get(&body.pose) {
                if let Some(active) = player.animation_mut(*index) {
                    let reference = catalog
                        .find(&body.id)
                        .and_then(|entry| {
                            if body.pose == Pose::Run {
                                entry.animations.run_speed
                            } else {
                                entry.animations.walk_speed
                            }
                        })
                        .unwrap_or(8.0);
                    active.set_speed((body.speed / reference).clamp(0.5, 2.0));
                }
            }
        }
    }
    for (bone, mut transform) in &mut bones {
        let Ok(body) = bodies.get(bone.owner) else {
            continue;
        };
        let moving = matches!(body.pose, Pose::Walk | Pose::Run);
        let swing = if moving {
            (time.elapsed_secs() * 10.0).sin() * 0.55
        } else {
            0.0
        };
        let turn = match bone.role {
            0 => Quat::from_rotation_z(1.15) * Quat::from_rotation_x(-swing),
            1 => Quat::from_rotation_z(-1.15) * Quat::from_rotation_x(swing),
            2 => Quat::from_rotation_x(swing),
            3 => Quat::from_rotation_x(-swing),
            4 => Quat::from_rotation_x(swing.max(0.0) * 0.7),
            5 => Quat::from_rotation_x((-swing).max(0.0) * 0.7),
            _ => Quat::from_rotation_x(if body.pose == Pose::Dash { 0.4 } else { 0.0 }),
        };
        *transform = bone.rest;
        transform.rotation = bone.rest.rotation * turn;
    }
}

#[derive(Component)]
struct ClothChain {
    child: Entity,
    owner: Entity,
}

fn pose_cloth(
    mut transforms: ParamSet<(TransformHelper, Query<&mut Transform>)>,
    chains: Query<(Entity, &ClothChain)>,
    parents: Query<&ChildOf>,
    owners: Query<&CharacterBody>,
) {
    for (root, chain) in &chains {
        if owners
            .get(chain.owner)
            .is_ok_and(|body| body.pose == Pose::Death)
        {
            continue;
        }
        let values = {
            let helper = transforms.p0();
            let Ok(parent) = parents.get(root) else {
                continue;
            };
            (
                helper.compute_global_transform(root),
                helper.compute_global_transform(chain.child),
                helper.compute_global_transform(parent.parent()),
            )
        };
        let (Ok(root_global), Ok(child), Ok(parent)) = values else {
            continue;
        };
        let current = (child.translation() - root_global.translation()).normalize_or_zero();
        if current == Vec3::ZERO {
            continue;
        }
        let delta = Quat::from_rotation_arc(current, Vec3::NEG_Y);
        if let Ok(mut transform) = transforms.p1().get_mut(root) {
            transform.rotation =
                (parent.rotation().inverse() * delta * root_global.rotation()).normalize();
        }
    }
}
