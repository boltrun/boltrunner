use bevy::{asset::RenderAssetUsages, mesh::PrimitiveTopology, prelude::*};
use boltrunner_shared::level;

pub struct MapDecorPlugin;
impl Plugin for MapDecorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DecorMap>().add_systems(
            Update,
            (
                build
                    .after(boltrunner_client::update_session)
                    .after(crate::map::build_map),
                rotate,
            )
                .chain(),
        );
    }
}
#[derive(Resource, Default)]
struct DecorMap(Option<&'static str>);
#[derive(Component)]
struct WindmillRotor {
    angular_speed: f32,
    base: Quat,
    angle: f32,
}

fn build(
    mut commands: Commands,
    session: Res<boltrunner_client::Session>,
    mut previous: ResMut<DecorMap>,
    old: Query<Entity, With<WindmillRotor>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    assets: Res<AssetServer>,
) {
    let Some(id) = session.map.as_deref() else {
        return;
    };
    if previous.0 == Some(id) {
        return;
    }
    for entity in &old {
        commands.entity(entity).despawn();
    }
    let map = level::active();
    previous.0 = Some(map.id);
    if map.data().windmills.is_empty() {
        return;
    }
    let Some(source) = &map.data().windmill_mesh else {
        return;
    };
    let mesh = meshes.add(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, source.positions.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, source.normals.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, source.uvs.clone()),
    );
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(
            assets.load(format!("{}/built/textures/{}", map.id, source.texture)),
        ),
        unlit: true,
        ..default()
    });
    for windmill in &map.data().windmills {
        let Some(axis) = windmill.axis.try_normalize() else {
            continue;
        };
        let base = Quat::from_rotation_arc(Vec3::Z, axis);
        commands.spawn((
            Name::new("Windmill sails"),
            WindmillRotor {
                angular_speed: windmill.angular_speed,
                base,
                angle: 0.0,
            },
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(windmill.hub)
                .with_rotation(base)
                .with_scale(Vec3::splat(windmill.scale)),
        ));
    }
}
fn rotate(time: Res<Time>, mut rotors: Query<(&mut WindmillRotor, &mut Transform)>) {
    for (mut rotor, mut transform) in &mut rotors {
        rotor.angle =
            (rotor.angle + rotor.angular_speed * time.delta_secs()) % std::f32::consts::TAU;
        transform.rotation = rotor.base * Quat::from_rotation_z(rotor.angle);
    }
}
