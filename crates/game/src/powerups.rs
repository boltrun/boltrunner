use bevy::{light::NotShadowCaster, prelude::*};
use boltrunner_shared::powerups::{Pickup, PickupKind};

pub struct PowerupVisualsPlugin;

pub(crate) fn pickup_color(kind: PickupKind) -> Color {
    match kind {
        PickupKind::Health => Color::srgb(0.20, 0.95, 0.43),
        PickupKind::Speed => Color::srgb(1.0, 0.72, 0.18),
        PickupKind::RapidFire => Color::srgb(0.76, 0.35, 1.0),
    }
}

impl Plugin for PowerupVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(
            Update,
            (attach, animate)
                .chain()
                .after(boltrunner_client::update_session),
        );
    }
}

#[derive(Resource)]
struct PickupAssets {
    cube: Handle<Mesh>,
    sphere: Handle<Mesh>,
    health: Handle<StandardMaterial>,
    speed: Handle<StandardMaterial>,
    rapid_fire: Handle<StandardMaterial>,
    highlight: Handle<StandardMaterial>,
}

#[derive(Component)]
struct PickupVisual;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let material = |color| StandardMaterial {
        base_color: color,
        unlit: true,
        ..default()
    };
    commands.insert_resource(PickupAssets {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        sphere: meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap()),
        health: materials.add(material(pickup_color(PickupKind::Health))),
        speed: materials.add(material(pickup_color(PickupKind::Speed))),
        rapid_fire: materials.add(material(pickup_color(PickupKind::RapidFire))),
        highlight: materials.add(material(Color::srgb(0.96, 0.98, 1.0))),
    });
}

fn attach(
    mut commands: Commands,
    assets: Res<PickupAssets>,
    pickups: Query<(Entity, &Pickup), Without<PickupVisual>>,
) {
    for (entity, pickup) in &pickups {
        commands.entity(entity).insert((
            PickupVisual,
            Transform::from_translation(pickup.position),
            Visibility::Inherited,
        ));
        commands
            .entity(entity)
            .with_children(|parent| match pickup.kind {
                PickupKind::Health => {
                    for scale in [Vec3::new(0.54, 0.17, 0.17), Vec3::new(0.17, 0.54, 0.17)] {
                        parent.spawn((
                            Mesh3d(assets.cube.clone()),
                            MeshMaterial3d(assets.health.clone()),
                            Transform::from_scale(scale),
                            NotShadowCaster,
                        ));
                    }
                }
                PickupKind::Speed => {
                    // A broad, jagged lightning bolt stays recognizable while rotating.
                    for (start, end) in [
                        (Vec2::new(0.18, 0.38), Vec2::new(-0.12, 0.04)),
                        (Vec2::new(-0.12, 0.04), Vec2::new(0.13, -0.06)),
                        (Vec2::new(0.13, -0.06), Vec2::new(-0.18, -0.38)),
                    ] {
                        let delta = end - start;
                        parent.spawn((
                            Mesh3d(assets.cube.clone()),
                            MeshMaterial3d(assets.speed.clone()),
                            Transform::from_translation(((start + end) * 0.5).extend(0.0))
                                .with_rotation(Quat::from_rotation_z(-delta.x.atan2(delta.y)))
                                .with_scale(Vec3::new(0.18, delta.length() + 0.08, 0.12)),
                            NotShadowCaster,
                        ));
                    }
                }
                PickupKind::RapidFire => {
                    for x in [-0.20, 0.0, 0.20] {
                        parent.spawn((
                            Mesh3d(assets.cube.clone()),
                            MeshMaterial3d(assets.rapid_fire.clone()),
                            Transform::from_translation(Vec3::new(x, 0.0, 0.0))
                                .with_scale(Vec3::new(0.12, 0.40, 0.15)),
                            NotShadowCaster,
                        ));
                        parent.spawn((
                            Mesh3d(assets.sphere.clone()),
                            MeshMaterial3d(assets.highlight.clone()),
                            Transform::from_translation(Vec3::new(x, 0.24, 0.0))
                                .with_scale(Vec3::splat(0.07)),
                            NotShadowCaster,
                        ));
                    }
                }
            });
    }
}

fn animate(time: Res<Time>, mut pickups: Query<(&Pickup, &mut Transform), With<PickupVisual>>) {
    let elapsed = time.elapsed_secs();
    for (pickup, mut transform) in &mut pickups {
        let phase = pickup.position.x * 0.3 + pickup.position.z * 0.2;
        transform.translation = pickup.position + Vec3::Y * ((elapsed * 2.6 + phase).sin() * 0.12);
        transform.rotation = Quat::from_rotation_y(elapsed * 1.4 + phase);
    }
}
