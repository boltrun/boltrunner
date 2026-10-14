use bevy::{light::NotShadowCaster, prelude::*};
use boltrunner_client::Session;
use boltrunner_shared::{
    PlayerState,
    level::{self, TriggerKind},
};
use lightyear::prelude::Predicted;

const CABLE_HEIGHT: f32 = 2.35;
pub struct ZiplineVisualPlugin;
impl Plugin for ZiplineVisualPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(
            Update,
            (cables, trolleys, hint)
                .chain()
                .after(crate::map::build_map)
                .after(crate::view::CameraUpdated),
        );
    }
}
#[derive(Resource)]
struct CableAssets {
    bar: Handle<Mesh>,
    box_mesh: Handle<Mesh>,
    dark: Handle<StandardMaterial>,
    gold: Handle<StandardMaterial>,
}
#[derive(Component)]
struct Trolley;
#[derive(Component)]
struct HasTrolley;
#[derive(Component)]
struct Hint;
#[derive(Component)]
struct HintText;
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(CableAssets {
        bar: meshes.add(Cylinder::new(1.0, 1.0).mesh().resolution(8)),
        box_mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        dark: materials.add(StandardMaterial {
            base_color: Color::srgb(0.22, 0.24, 0.31),
            perceptual_roughness: 0.6,
            ..default()
        }),
        gold: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.73, 0.23),
            unlit: true,
            ..default()
        }),
    });
    commands
        .spawn((
            Hint,
            Node {
                position_type: PositionType::Absolute,
                bottom: px(88),
                width: percent(100),
                justify_content: JustifyContent::Center,
                display: Display::None,
                ..default()
            },
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    padding: UiRect::axes(px(12), px(7)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.04, 0.06, 0.09, 0.85)),
            ))
            .with_child((
                HintText,
                Text::new(""),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.86, 0.48)),
            ));
        });
}
fn cables(
    mut commands: Commands,
    session: Res<Session>,
    assets: Res<CableAssets>,
    mut built: Local<Option<String>>,
) {
    let Some(id) = session.map.as_deref() else {
        return;
    };
    if built.as_deref() == Some(id) || level::active().id != id {
        return;
    }
    *built = Some(id.into());
    for (index, trigger) in level::data().triggers.iter().enumerate() {
        let TriggerKind::Zipline { start, end } = trigger.kind else {
            continue;
        };
        let duplicate=level::data().triggers[..index].iter().any(|other| {
            matches!(other.kind, TriggerKind::Zipline {start:a,end:b} if (a==start&&b==end)||(a==end&&b==start))
        });
        if duplicate {
            continue;
        }
        let delta = end - start;
        commands.spawn((
            crate::map::MapGeometry,
            Mesh3d(assets.bar.clone()),
            MeshMaterial3d(assets.dark.clone()),
            Transform::from_translation((start + end) * 0.5 + Vec3::Y * CABLE_HEIGHT)
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, delta.normalize()))
                .with_scale(Vec3::new(0.045, delta.length(), 0.045)),
            NotShadowCaster,
        ));
        for point in [start, end] {
            commands.spawn((
                crate::map::MapGeometry,
                Mesh3d(assets.box_mesh.clone()),
                MeshMaterial3d(assets.gold.clone()),
                Transform::from_translation(point + Vec3::Y * (CABLE_HEIGHT - 0.12))
                    .with_scale(Vec3::new(0.30, 0.22, 0.25)),
                NotShadowCaster,
            ));
        }
    }
}
fn trolleys(
    mut commands: Commands,
    assets: Res<CableAssets>,
    players: Query<Entity, (With<crate::view::PlayerVisual>, Without<HasTrolley>)>,
    states: Query<&PlayerState>,
    mut cars: Query<(&ChildOf, &mut Transform, &mut Visibility), With<Trolley>>,
) {
    for entity in &players {
        commands
            .entity(entity)
            .insert(HasTrolley)
            .with_children(|p| {
                p.spawn((Trolley, Transform::default(), Visibility::Hidden))
                    .with_children(|p| {
                        p.spawn((
                            Mesh3d(assets.box_mesh.clone()),
                            MeshMaterial3d(assets.gold.clone()),
                            Transform::from_scale(Vec3::new(0.20, 0.14, 0.27)),
                            NotShadowCaster,
                        ));
                        p.spawn((
                            Mesh3d(assets.bar.clone()),
                            MeshMaterial3d(assets.dark.clone()),
                            Transform::from_xyz(0.0, -0.27, 0.0)
                                .with_scale(Vec3::new(0.027, 0.50, 0.027)),
                            NotShadowCaster,
                        ));
                    });
            });
    }
    for (parent, mut transform, mut visible) in &mut cars {
        let Ok(state) = states.get(parent.parent()) else {
            continue;
        };
        let Some(ride) = state.zipline.ride else {
            *visible = Visibility::Hidden;
            continue;
        };
        let Some(trigger) = level::data().triggers.get(ride.trigger as usize) else {
            *visible = Visibility::Hidden;
            continue;
        };
        let TriggerKind::Zipline { start, end } = trigger.kind else {
            continue;
        };
        let direction = if ride.reverse {
            start - end
        } else {
            end - start
        };
        *visible = Visibility::Inherited;
        *transform = Transform::from_translation(Vec3::Y * (CABLE_HEIGHT - 0.1)).with_rotation(
            Quat::from_rotation_y(-state.yaw - direction.x.atan2(-direction.z)),
        );
    }
}
fn hint(
    session: Res<Session>,
    look: Res<crate::view::Look>,
    local: Query<&PlayerState, With<Predicted>>,
    mut panel: Single<&mut Node, With<Hint>>,
    mut text: Single<&mut Text, With<HintText>>,
) {
    let message = local.iter().next().and_then(|state| {
        if !session.is_playing()
            || !look.locked
            || state.death.is_some()
            || state.match_paused
            || state.drone.is_some_and(|d| d.controlling)
        {
            return None;
        }
        if state.zipline.ride.is_some() {
            Some("Space / Shift — jump off")
        } else if boltrunner_shared::zipline::nearby(level::world(), &level::data().triggers, state)
            .is_some()
        {
            Some("E — ride zipline")
        } else {
            None
        }
    });
    panel.display = if message.is_some() {
        Display::Flex
    } else {
        Display::None
    };
    text.0 = message.unwrap_or_default().into();
}
