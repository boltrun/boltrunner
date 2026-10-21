use bevy::{
    image::{ImageLoaderSettings, ImageSampler},
    light::NotShadowCaster,
    prelude::*,
};
use boltrunner_shared::crystals::{Crystal, CrystalExplosion};
use lightyear::prelude::*;

pub struct CrystalVisualsPlugin;
impl Plugin for CrystalVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(Update, (attach, animate, explosions, fade).chain());
    }
}

#[derive(Resource)]
struct CrystalAssets {
    cube: Handle<Mesh>,
    blast_frames: Vec<Handle<Mesh>>,
    smoke: Handle<StandardMaterial>,
    core: Handle<StandardMaterial>,
    pink: Handle<StandardMaterial>,
    frame: Handle<StandardMaterial>,
    glow: Handle<StandardMaterial>,
}
#[derive(Component)]
struct CrystalVisual;
#[derive(Component)]
struct CrystalCage;
#[derive(Component)]
struct Burst {
    age: f32,
    velocity: Vec3,
    size: f32,
    delay: f32,
    map_revision: u64,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    let opaque = |color| StandardMaterial {
        base_color: color,
        unlit: true,
        ..default()
    };
    let blast_frames = (0..16)
        .map(|frame| {
            let mut mesh = Mesh::from(Rectangle::new(1., 1.));
            if let Some(bevy::mesh::VertexAttributeValues::Float32x2(uvs)) =
                mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0)
            {
                for uv in uvs {
                    uv[0] = (frame as f32 % 4. + (0.5 + uv[0] * 31.) / 32.) / 4.;
                    uv[1] = ((frame / 4) as f32 + (0.5 + uv[1] * 31.) / 32.) / 4.;
                }
            }
            meshes.add(mesh)
        })
        .collect();
    commands.insert_resource(CrystalAssets {
        cube: meshes.add(Cuboid::new(1., 1., 1.)),
        blast_frames,
        smoke: materials.add(StandardMaterial {
            base_color_texture: Some(
                asset_server
                    .load_builder()
                    .with_settings(|settings: &mut ImageLoaderSettings| {
                        settings.sampler = ImageSampler::nearest();
                    })
                    .load("bottle_shipyard/built/textures/crystal_explosion.png"),
            ),
            unlit: true,
            alpha_mode: AlphaMode::Mask(0.5),
            cull_mode: None,
            ..default()
        }),
        core: materials.add(opaque(Color::srgb(1., 0.79, 1.))),
        pink: materials.add(opaque(Color::srgb(0.91, 0.13, 0.71))),
        frame: materials.add(opaque(Color::srgb(0.90, 0.94, 1.))),
        glow: materials.add(StandardMaterial {
            base_color: Color::srgba(0.92, 0.21, 0.92, 0.24),
            unlit: true,
            alpha_mode: AlphaMode::Add,
            ..default()
        }),
    });
}

fn attach(
    mut commands: Commands,
    assets: Res<CrystalAssets>,
    crystals: Query<(Entity, &Crystal), Without<CrystalVisual>>,
) {
    for (entity, crystal) in &crystals {
        commands
            .entity(entity)
            .insert((
                CrystalVisual,
                Transform::from_translation(crystal.position),
                Visibility::Inherited,
            ))
            .with_children(|parent| {
                parent
                    .spawn((CrystalCage, Transform::default(), Visibility::Inherited))
                    .with_children(|cage| {
                        // Three nested cubes and a white open cage reproduce the End crystal silhouette.
                        for (size, material, rotation) in [
                            (0.62, &assets.core, Quat::IDENTITY),
                            (0.93, &assets.pink, Quat::from_rotation_z(0.63)),
                            (1.32, &assets.glow, Quat::from_rotation_x(0.4)),
                        ] {
                            cage.spawn((
                                Mesh3d(assets.cube.clone()),
                                MeshMaterial3d(material.clone()),
                                Transform::from_scale(Vec3::splat(size)).with_rotation(rotation),
                                NotShadowCaster,
                            ));
                        }
                        for axis in 0..3 {
                            for a in [-0.72, 0.72] {
                                for b in [-0.72, 0.72] {
                                    let mut p = Vec3::ZERO;
                                    let mut scale = Vec3::splat(0.055);
                                    p[(axis + 1) % 3] = a;
                                    p[(axis + 2) % 3] = b;
                                    scale[axis] = 1.49;
                                    cage.spawn((
                                        Mesh3d(assets.cube.clone()),
                                        MeshMaterial3d(assets.frame.clone()),
                                        Transform::from_translation(p).with_scale(scale),
                                        NotShadowCaster,
                                    ));
                                }
                            }
                        }
                    });
            });
    }
}

fn animate(
    time: Res<Time>,
    mut crystals: Query<(&Crystal, &Children, &mut Visibility), With<CrystalVisual>>,
    mut cages: Query<&mut Transform, With<CrystalCage>>,
) {
    for (crystal, children, mut visibility) in &mut crystals {
        *visibility = if crystal.active {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        for child in children {
            if let Ok(mut transform) = cages.get_mut(*child) {
                transform.rotation =
                    Quat::from_euler(EulerRot::YXZ, time.elapsed_secs() * 0.55, 0.52, 0.37);
                transform.translation.y = (time.elapsed_secs() * 1.7).sin() * 0.06;
            }
        }
    }
}

fn explosions(
    mut commands: Commands,
    assets: Res<CrystalAssets>,
    session: Res<boltrunner_client::Session>,
    mut links: Query<&mut MessageReceiver<CrystalExplosion>, With<Connected>>,
) {
    for mut receiver in &mut links {
        for event in receiver.receive() {
            if session.map_revision != Some(event.map_revision) {
                continue;
            }
            // Brief white core followed by staggered, expanding pixel smoke puffs.
            for i in 0..17 {
                let angle = i as f32 * 2.399963;
                let y = 1. - 2. * (i as f32 + 0.5) / 17.;
                let radial = (1. - y * y).sqrt();
                let direction = Vec3::new(radial * angle.cos(), y, radial * angle.sin());
                let distance = if i == 0 { 0. } else { event.radius * 0.3 };
                commands.spawn((
                    Burst {
                        age: 0.,
                        velocity: direction * 1.2,
                        size: if i == 0 {
                            4.5
                        } else {
                            2.8 + (i % 3) as f32 * 0.45
                        },
                        delay: if i == 0 { 0. } else { 0.035 * (i % 4) as f32 },
                        map_revision: event.map_revision,
                    },
                    Mesh3d(assets.blast_frames[0].clone()),
                    MeshMaterial3d(assets.smoke.clone()),
                    Transform::from_translation(event.position + direction * distance),
                    Visibility::Hidden,
                    NotShadowCaster,
                ));
            }
        }
    }
}

fn fade(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<CrystalAssets>,
    session: Res<boltrunner_client::Session>,
    camera: Query<&GlobalTransform, With<crate::view::PlayerCamera>>,
    mut bursts: Query<(
        Entity,
        &mut Burst,
        &mut Transform,
        &mut Mesh3d,
        &mut Visibility,
    )>,
) {
    let camera_rotation = camera.iter().next().map(|camera| camera.rotation());
    for (entity, mut burst, mut transform, mut mesh, mut visibility) in &mut bursts {
        burst.age += time.delta_secs();
        let age = burst.age - burst.delay;
        if age >= 0.65 || session.map_revision != Some(burst.map_revision) {
            commands.entity(entity).despawn();
            continue;
        }
        if age < 0. {
            continue;
        }
        *visibility = Visibility::Inherited;
        mesh.0 = assets.blast_frames[((age / 0.65 * 16.) as usize).min(15)].clone();
        transform.translation += burst.velocity * time.delta_secs();
        transform.scale = Vec3::splat(burst.size * (0.7 + age * 0.8));
        if let Some(rotation) = camera_rotation {
            transform.rotation = rotation;
        }
    }
}
