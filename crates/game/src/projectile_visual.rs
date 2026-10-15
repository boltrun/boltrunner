use bevy::{
    asset::RenderAssetUsages,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use boltrunner_shared::{arsenal::WeaponKind, weapon::Projectile};

pub struct ProjectileVisualPlugin;
impl Plugin for ProjectileVisualPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(Update, animate.after(crate::weapon::WeaponVisuals));
    }
}

#[derive(Resource)]
pub(crate) struct ProjectileAssets {
    sphere: Handle<Mesh>,
    ring: Handle<Mesh>,
    lightning: Handle<Mesh>,
    trail: Handle<Mesh>,
    seeker: Handle<StandardMaterial>,
    electric: Handle<StandardMaterial>,
    blue: Handle<StandardMaterial>,
    white: Handle<StandardMaterial>,
    green: Handle<StandardMaterial>,
    colors: Vec<Handle<StandardMaterial>>,
}

#[derive(Component)]
struct EnergyMotion {
    rest: Transform,
    phase: f32,
    speed: f32,
    flicker: bool,
}

fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let solid = |texture| StandardMaterial {
        base_color_texture: Some(assets.load(texture)),
        unlit: true,
        ..default()
    };
    let glow = |color| StandardMaterial {
        base_color: color,
        unlit: true,
        alpha_mode: AlphaMode::Add,
        cull_mode: None,
        ..default()
    };
    let colors = [
        [1.0, 0.23, 0.32],
        [0.15, 0.68, 1.0],
        [1.0, 0.75, 0.12],
        [0.67, 0.28, 1.0],
        [0.14, 1.0, 0.55],
        [1.0, 0.37, 0.08],
        [0.97, 0.19, 0.78],
        [0.12, 0.95, 0.98],
    ]
    .into_iter()
    .map(|[r, g, b]| {
        materials.add(StandardMaterial {
            base_color: Color::srgb(r, g, b),
            emissive: LinearRgba::new(r * 0.25, g * 0.25, b * 0.25, 1.0),
            metallic: 0.15,
            perceptual_roughness: 0.24,
            ..default()
        })
    })
    .collect();
    commands.insert_resource(ProjectileAssets {
        sphere: meshes.add(Sphere::new(1.0).mesh().uv(20, 12)),
        ring: meshes.add(
            Torus::new(0.88, 0.96)
                .mesh()
                .major_resolution(20)
                .minor_resolution(6),
        ),
        lightning: meshes.add(lightning_mesh()),
        trail: meshes.add(Cone::new(1.0, 1.0)),
        seeker: materials.add(solid("weapons/arsenal/built/seeker-energy-v1.png")),
        electric: materials.add(solid("weapons/arsenal/built/electro-energy-v1.png")),
        blue: materials.add(glow(Color::srgba(0.06, 0.48, 1.0, 0.72))),
        white: materials.add(glow(Color::srgba(0.65, 0.91, 1.0, 0.85))),
        green: materials.add(glow(Color::srgba(0.22, 1.0, 0.32, 0.30))),
        colors,
    });
}

// Three jagged branches in different planes, joined into one reusable mesh.
// Crossed ribbons remain visible when viewed along any flight direction.
fn lightning_mesh() -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for branch in 0..3 {
        let angle = branch as f32 * std::f32::consts::TAU / 3.0;
        let radial = Vec3::new(angle.cos(), angle.sin(), 0.0);
        let tangent = Vec3::new(-angle.sin(), angle.cos(), 0.0);
        let point = |step: usize| {
            let t = step as f32 / 8.0;
            let envelope = (t * std::f32::consts::PI).sin();
            let zigzag = if step % 2 == 0 { 0.18 } else { -0.15 };
            radial * ((0.50 + zigzag) * envelope)
                + tangent * (zigzag * envelope)
                + Vec3::Z * (t * 2.0 - 1.0)
        };
        for step in 0..8 {
            let a = point(step);
            let b = point(step + 1);
            for side in [radial, tangent] {
                let side = side * 0.022;
                let first = vertices.len() as u32;
                vertices
                    .extend([a - side, a + side, b + side, b - side].map(|point| point.to_array()));
                indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
            }
        }
    }
    let normals = vec![[0.0, 1.0, 0.0]; vertices.len()];
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vertices)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(Indices::U32(indices))
}

pub(crate) fn spawn(
    parent: &mut ChildSpawnerCommands,
    assets: &ProjectileAssets,
    bolt: &Projectile,
) -> bool {
    let r = bolt.radius;
    // Consecutive shots cycle the palette; identity, not local randomness,
    // keeps a ball's color stable through prediction, replication and bounces.
    let color = (bolt.shot as usize + (bolt.owner % 8) as usize + bolt.pellet as usize)
        % assets.colors.len();
    let phase = (bolt.shot as f32 * 0.73 + (bolt.owner % 31) as f32) % std::f32::consts::TAU;
    let mut part = |mesh: &Handle<Mesh>,
                    material: &Handle<StandardMaterial>,
                    transform: Transform,
                    speed: f32,
                    flicker: bool| {
        let mut entity = parent.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            transform,
            NotShadowCaster,
        ));
        if speed != 0.0 {
            entity.insert(EnergyMotion {
                rest: transform,
                phase,
                speed,
                flicker,
            });
        }
    };
    match bolt.kind {
        WeaponKind::Ricochet => {
            part(
                &assets.sphere,
                &assets.colors[color],
                Transform::from_scale(Vec3::splat(r)),
                2.0,
                false,
            );
            part(
                &assets.ring,
                &assets.white,
                Transform::from_rotation(Quat::from_rotation_z(0.6))
                    .with_scale(Vec3::splat(r * 1.09)),
                3.0,
                false,
            );
        }
        WeaponKind::HomingPlasma => {
            part(
                &assets.sphere,
                &assets.seeker,
                Transform::from_scale(Vec3::splat(r * 0.86)),
                3.5,
                false,
            );
            for angle in [-0.8, 0.8] {
                part(
                    &assets.ring,
                    &assets.green,
                    Transform::from_rotation(Quat::from_rotation_z(angle))
                        .with_scale(Vec3::splat(r * 1.03)),
                    angle * 2.0,
                    false,
                );
            }
            part(
                &assets.trail,
                &assets.green,
                Transform::from_xyz(0.0, 0.0, r * 1.5)
                    .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
                    .with_scale(Vec3::new(r * 0.42, r * 3.0, r * 0.42)),
                0.0,
                false,
            );
        }
        WeaponKind::ElectroPlasma => {
            part(
                &assets.sphere,
                &assets.electric,
                Transform::from_scale(Vec3::splat(r * 0.56)),
                4.0,
                false,
            );
            part(
                &assets.lightning,
                &assets.white,
                Transform::from_scale(Vec3::splat(r)),
                15.0,
                true,
            );
            part(
                &assets.lightning,
                &assets.blue,
                Transform::from_xyz(0.0, 0.0, r * 1.7)
                    .with_rotation(Quat::from_rotation_z(1.1))
                    .with_scale(Vec3::new(r * 1.3, r * 1.3, r * 2.7)),
                -18.0,
                true,
            );
        }
        _ => return false,
    }
    true
}

fn animate(time: Res<Time>, mut parts: Query<(&EnergyMotion, &mut Transform)>) {
    for (motion, mut transform) in &mut parts {
        *transform = motion.rest;
        let t = time.elapsed_secs();
        let angle = if motion.flicker {
            (t * motion.speed.abs()).floor() * 2.39996 + motion.phase
        } else {
            t * motion.speed + motion.phase
        };
        transform.rotation *= Quat::from_euler(EulerRot::ZYX, angle, angle * 0.38, 0.0);
        if motion.flicker {
            // Only rotate electric branches; the solid core never changes the
            // apparent collision radius or pulses into a giant muzzle flash.
            transform.rotation = motion.rest.rotation * Quat::from_rotation_z(angle);
        }
    }
}
