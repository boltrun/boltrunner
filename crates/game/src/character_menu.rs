use crate::characters::{CharacterBody, CharacterModels, Pose};
use bevy::{
    camera::{RenderTarget, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    prelude::*,
    render::render_resource::TextureFormat,
};
use boltrunner_client::{Session, SessionPhase, characters::CharacterSelection};
use boltrunner_shared::{characters::CharacterCatalog, tuning::SimulationTuning};

#[derive(Resource, Default)]
pub struct CharacterMenu {
    pub open: bool,
    selected: String,
    pose: Pose,
    hitbox: bool,
    initialized: bool,
    rotating: bool,
    yaw: f32,
}
#[derive(Component)]
struct MenuRoot;
#[derive(Component)]
struct List;
#[derive(Component)]
struct Choice(String);
#[derive(Component)]
struct UseCharacter;
#[derive(Component)]
struct UseLabel;
#[derive(Component)]
struct Close;
#[derive(Component)]
struct PoseChoice(Pose);
#[derive(Component)]
struct HitboxToggle;
#[derive(Component)]
struct HitboxLabel;
#[derive(Component)]
struct Rotate;
#[derive(Component)]
struct RotateLabel;
#[derive(Component)]
struct Details;
#[derive(Component)]
struct Preview;
#[derive(Component)]
struct PreviewCamera;
#[derive(Component)]
struct PreviewHitbox;
#[derive(Component)]
struct PreviewFallback;
#[derive(Component)]
pub(crate) struct OpenCharacters;
#[derive(Component)]
pub(crate) struct SelectedCharacterLabel;

pub struct CharacterMenuPlugin;
impl Plugin for CharacterMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CharacterMenu>()
            .add_systems(Startup, setup)
            .add_systems(
                PreUpdate,
                hotkeys
                    .after(crate::debug_menu::MenuInput)
                    .before(crate::view::look_input),
            )
            .add_systems(
                Update,
                (list, actions, present, preview_weapon, preview_shapes)
                    .chain()
                    .after(boltrunner_client::update_session),
            );
    }
}

fn button(parent: &mut ChildSpawnerCommands, label: &str, marker: impl Bundle) {
    parent
        .spawn((
            Button,
            marker,
            Node {
                min_height: px(40),
                padding: UiRect::axes(px(14), px(8)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.12, 0.22, 0.28)),
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                TextFont {
                    font_size: 17.0,
                    ..default()
                },
            ));
        });
}

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let image = images.add(Image::new_target_texture(
        640,
        640,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: -1,
            is_active: false,
            clear_color: Color::srgb(0.055, 0.08, 0.115).into(),
            ..default()
        },
        RenderTarget::Image(image.clone().into()),
        PreviewCamera,
        RenderLayers::layer(2),
        Transform::from_xyz(0.0, 1.05, 3.5).looking_at(Vec3::new(0.0, 0.95, 0.0), Vec3::Y),
        Projection::Perspective(PerspectiveProjection {
            fov: 40.0_f32.to_radians(),
            ..default()
        }),
        Tonemapping::None,
        Msaa::Sample4,
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 8000.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.6, -0.8, 0.0)),
        RenderLayers::layer(2),
    ));
    let tuning = SimulationTuning::default();
    commands
        .spawn((
            Preview,
            boltrunner_shared::equipment::PlayerEquipment::default(),
            crate::world_weapon::WeaponGrip {
                height: 0.70,
                forward: 0.45,
                side: 0.27,
                two_handed: true,
                preview: true,
                simple_arms: true,
            },
            Transform::default(),
            Visibility::Hidden,
            CharacterBody {
                id: String::new(),
                height: tuning.player_height,
                pose: Pose::Idle,
                speed: 0.0,
                preview: true,
            },
        ))
        .with_children(|root| {
            root.spawn((
                crate::characters::FallbackCharacter,
                PreviewFallback,
                Mesh3d(meshes.add(Capsule3d::new(
                    tuning.player_radius,
                    tuning.player_height - 2.0 * tuning.player_radius,
                ))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.3, 0.8, 0.85),
                    ..default()
                })),
                Transform::from_xyz(0.0, tuning.player_height / 2.0, 0.0),
                RenderLayers::layer(2),
            ));
            root.spawn((
                PreviewHitbox,
                Mesh3d(meshes.add(Capsule3d::new(
                    tuning.player_radius,
                    tuning.player_height - 2.0 * tuning.player_radius,
                ))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgba(0.4, 1.0, 0.7, 0.2),
                    alpha_mode: AlphaMode::Blend,
                    unlit: true,
                    cull_mode: None,
                    ..default()
                })),
                Transform::from_xyz(0.0, tuning.player_height / 2.0, 0.0),
                Visibility::Hidden,
                RenderLayers::layer(2),
            ));
        });
    commands
        .spawn((
            MenuRoot,
            GlobalZIndex(1000),
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.02, 0.04, 0.94)),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: percent(92),
                    max_width: px(960),
                    max_height: percent(94),
                    padding: UiRect::all(px(20)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(14),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.045, 0.065, 0.09)),
            ))
            .with_children(|panel| {
                panel
                    .spawn(Node {
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|header| {
                        header.spawn((
                            Text::new("Characters"),
                            TextFont {
                                font_size: 28.0,
                                ..default()
                            },
                        ));
                        button(header, "Close", Close);
                    });
                panel
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: px(20),
                        row_gap: px(14),
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn(Node {
                            flex_direction: FlexDirection::Column,
                            flex_grow: 1.0,
                            flex_basis: px(300),
                            min_width: px(240),
                            row_gap: px(12),
                            ..default()
                        })
                        .with_children(|preview| {
                            preview.spawn((
                                ImageNode::new(image),
                                Node {
                                    width: percent(100),
                                    aspect_ratio: Some(1.0),
                                    max_height: px(460),
                                    ..default()
                                },
                            ));
                            preview
                                .spawn(Node {
                                    flex_wrap: FlexWrap::Wrap,
                                    column_gap: px(6),
                                    row_gap: px(6),
                                    ..default()
                                })
                                .with_children(|poses| {
                                    for (label, pose) in [
                                        ("Idle", Pose::Idle),
                                        ("Walk", Pose::Walk),
                                        ("Run", Pose::Run),
                                        ("Dash", Pose::Dash),
                                    ] {
                                        button(poses, label, PoseChoice(pose));
                                    }
                                    poses
                                        .spawn((
                                            Button,
                                            Rotate,
                                            Node {
                                                min_height: px(40),
                                                padding: UiRect::axes(px(14), px(8)),
                                                ..default()
                                            },
                                            BackgroundColor(Color::srgb(0.12, 0.22, 0.28)),
                                        ))
                                        .with_child((
                                            RotateLabel,
                                            Text::new("Rotate"),
                                            TextFont {
                                                font_size: 17.0,
                                                ..default()
                                            },
                                        ));
                                });
                            preview
                                .spawn((
                                    Button,
                                    HitboxToggle,
                                    Node {
                                        padding: UiRect::all(px(10)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(0.1, 0.18, 0.23)),
                                ))
                                .with_children(|toggle| {
                                    toggle.spawn((
                                        HitboxLabel,
                                        Text::new("Show hitbox"),
                                        TextFont {
                                            font_size: 16.0,
                                            ..default()
                                        },
                                    ));
                                });
                        });
                        row.spawn(Node {
                            flex_direction: FlexDirection::Column,
                            flex_grow: 1.0,
                            flex_basis: px(300),
                            min_width: px(240),
                            row_gap: px(14),
                            ..default()
                        })
                        .with_children(|right| {
                            right.spawn((
                                List,
                                Node {
                                    display: Display::Grid,
                                    grid_template_columns: vec![
                                        GridTrack::fr(1.0),
                                        GridTrack::fr(1.0),
                                    ],
                                    column_gap: px(8),
                                    row_gap: px(8),
                                    ..default()
                                },
                            ));
                            right.spawn((
                                Details,
                                Text::new(""),
                                TextFont {
                                    font_size: 15.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.65, 0.77, 0.85)),
                            ));
                            right
                                .spawn((
                                    Button,
                                    UseCharacter,
                                    Node {
                                        min_height: px(48),
                                        align_items: AlignItems::Center,
                                        justify_content: JustifyContent::Center,
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(0.12, 0.42, 0.46)),
                                ))
                                .with_children(|button| {
                                    button.spawn((
                                        UseLabel,
                                        Text::new("Use character"),
                                        TextFont {
                                            font_size: 19.0,
                                            ..default()
                                        },
                                    ));
                                });
                        });
                    });
            });
        });
}

pub(crate) fn hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    session: Res<Session>,
    mut menu: ResMut<CharacterMenu>,
    mut debug: ResMut<crate::debug_menu::DebugMenu>,
    maps: Res<crate::map_menu::MapMenu>,
    weapons: Res<crate::weapon_menu::WeaponMenu>,
) {
    let was_open = menu.open;
    if keys.just_pressed(KeyCode::F1)
        && matches!(session.phase, SessionPhase::Title | SessionPhase::Playing)
    {
        menu.open = !menu.open;
        if menu.open {
            menu.selected = session.character.clone();
            debug.open = false;
        }
    }
    if keys.just_pressed(KeyCode::Escape)
        || keys.just_pressed(KeyCode::F2)
        || keys.just_pressed(KeyCode::F3)
        || keys.just_pressed(KeyCode::F4)
    {
        menu.open = false;
    }
    if menu.open != was_open {
        crate::platform::set_debug_menu(menu.open || debug.open || maps.open || weapons.open);
    }
}

fn list(mut commands: Commands, catalog: Res<CharacterCatalog>, root: Single<Entity, With<List>>) {
    if !catalog.is_changed() {
        return;
    }
    commands
        .entity(*root)
        .despawn_children()
        .with_children(|list| {
            for (id, name) in std::iter::once(("", "Runner")).chain(
                catalog
                    .characters
                    .iter()
                    .map(|entry| (entry.id.as_str(), entry.name.as_str())),
            ) {
                button(list, name, Choice(id.into()));
            }
        });
}

fn actions(
    mut menu: ResMut<CharacterMenu>,
    mut session: ResMut<Session>,
    mut selection: ResMut<CharacterSelection>,
    buttons: Query<
        (
            &Interaction,
            Option<&Choice>,
            Option<&PoseChoice>,
            Has<UseCharacter>,
            Has<Close>,
            Has<HitboxToggle>,
            Has<OpenCharacters>,
            Has<Rotate>,
        ),
        Changed<Interaction>,
    >,
) {
    for (interaction, choice, pose, apply, close, hitbox, open, rotate) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if open {
            menu.open = true;
            menu.selected = session.character.clone();
            crate::platform::set_debug_menu(true);
        }
        if !menu.open {
            continue;
        }
        if let Some(choice) = choice {
            menu.selected = choice.0.clone();
            menu.pose = Pose::Idle;
            menu.yaw = 0.0;
        }
        if let Some(pose) = pose {
            menu.pose = pose.0;
        }
        if hitbox {
            menu.hitbox = !menu.hitbox;
        }
        if rotate {
            menu.rotating = !menu.rotating;
        }
        if apply && !selection.pending {
            if session.phase == SessionPhase::Title {
                session.character = menu.selected.clone();
                menu.open = false;
                crate::platform::set_debug_menu(false);
            } else if session.is_playing() {
                selection.choose(&menu.selected);
            }
        }
        if close {
            menu.open = false;
            crate::platform::set_debug_menu(false);
        }
    }
}

fn present(
    mut menu: ResMut<CharacterMenu>,
    mut session: ResMut<Session>,
    selection: Res<CharacterSelection>,
    catalog: Res<CharacterCatalog>,
    models: Res<CharacterModels>,
    tuning: Res<boltrunner_client::tuning::TuningClient>,
    time: Res<Time>,
    mut root: Single<&mut Node, With<MenuRoot>>,
    mut camera: Single<&mut Camera, With<PreviewCamera>>,
    mut preview: Single<(&mut CharacterBody, &mut Visibility, &mut Transform), With<Preview>>,
    mut details: Single<
        &mut Text,
        (
            With<Details>,
            Without<UseLabel>,
            Without<SelectedCharacterLabel>,
            Without<HitboxLabel>,
        ),
    >,
    mut use_label: Single<
        &mut Text,
        (
            With<UseLabel>,
            Without<Details>,
            Without<SelectedCharacterLabel>,
            Without<HitboxLabel>,
        ),
    >,
    mut title_label: Query<
        (&mut Text, Has<RotateLabel>),
        (
            Or<(With<SelectedCharacterLabel>, With<RotateLabel>)>,
            Without<Details>,
            Without<UseLabel>,
            Without<HitboxLabel>,
        ),
    >,
    mut hitbox_label: Single<
        &mut Text,
        (
            With<HitboxLabel>,
            Without<Details>,
            Without<UseLabel>,
            Without<SelectedCharacterLabel>,
        ),
    >,
    mut hitbox: Single<&mut Visibility, (With<PreviewHitbox>, Without<Preview>)>,
    mut choices: Query<(&Choice, &mut BackgroundColor)>,
) {
    if !menu.initialized && !catalog.characters.is_empty() && session.phase == SessionPhase::Title {
        session.character = catalog.characters[0].id.clone();
        menu.selected = session.character.clone();
        menu.initialized = true;
    }
    if !matches!(session.phase, SessionPhase::Title | SessionPhase::Playing) {
        menu.open = false;
    }
    root.display = if menu.open {
        Display::Flex
    } else {
        Display::None
    };
    camera.is_active = menu.open;
    let height = tuning.values().simulation.player_height;
    preview.0.id = if menu.open {
        menu.selected.clone()
    } else {
        String::new()
    };
    preview.0.height = height;
    preview.0.pose = menu.pose;
    preview.0.speed = if menu.pose == Pose::Run { 10.0 } else { 5.0 };
    *preview.1 = if menu.open {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if menu.open && menu.rotating {
        menu.yaw += time.delta_secs() * 0.5;
    }
    preview.2.rotation = Quat::from_rotation_y(std::f32::consts::PI + menu.yaw);
    **hitbox = if menu.hitbox {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    hitbox_label.0 = if menu.hitbox {
        "Hide hitbox"
    } else {
        "Show hitbox"
    }
    .into();
    let entry = catalog.find(&menu.selected);
    details.0 = entry
        .map(|entry| {
            format!(
                "{}\n\n{}\n\n{}",
                entry.name,
                entry.author,
                models.status(entry)
            )
        })
        .unwrap_or_else(|| "Runner".into());
    if let Some(error) = &selection.error {
        details.0.push_str(&format!("\n{error}"));
    }
    use_label.0 = if selection.pending {
        "Changing…"
    } else if session.character == menu.selected {
        "Selected"
    } else {
        "Use character"
    }
    .into();
    let name = catalog
        .find(&session.character)
        .map(|entry| entry.name.as_str())
        .unwrap_or("Runner");
    for (mut text, rotate) in &mut title_label {
        text.0 = if rotate {
            if menu.rotating { "Stop" } else { "Rotate" }.into()
        } else {
            format!("{name}  ›")
        };
    }
    for (choice, mut color) in &mut choices {
        color.0 = if choice.0 == menu.selected {
            Color::srgb(0.13, 0.4, 0.46)
        } else {
            Color::srgb(0.1, 0.16, 0.22)
        };
    }
}

fn preview_shapes(
    tuning: Res<boltrunner_client::tuning::TuningClient>,
    mut size: Local<Option<(f32, f32)>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut shapes: Query<
        (&mut Mesh3d, &mut Transform),
        Or<(With<PreviewHitbox>, With<PreviewFallback>)>,
    >,
) {
    let tuning = tuning.values().simulation;
    let next = (tuning.player_radius, tuning.player_height);
    if *size == Some(next) {
        return;
    }
    *size = Some(next);
    let mesh = meshes.add(Capsule3d::new(next.0, next.1 - 2.0 * next.0));
    for (mut shape, mut transform) in &mut shapes {
        shape.0 = mesh.clone();
        transform.translation.y = next.1 / 2.0;
    }
}

fn preview_weapon(
    menu: Res<CharacterMenu>,
    catalog: Res<CharacterCatalog>,
    mut preview: Single<
        (
            &mut boltrunner_shared::equipment::PlayerEquipment,
            &mut crate::world_weapon::WeaponGrip,
        ),
        With<Preview>,
    >,
    local: Query<
        &boltrunner_shared::equipment::PlayerEquipment,
        (With<lightyear::prelude::Predicted>, Without<Preview>),
    >,
) {
    *preview.0 = local.iter().next().copied().unwrap_or_default();
    preview.1.simple_arms = catalog.find(&menu.selected).is_none();
    preview.1.two_handed = catalog
        .find(&menu.selected)
        .and_then(|e| e.arms.as_ref())
        .is_none_or(|a| a.two_handed);
    preview.1.forward = catalog
        .find(&menu.selected)
        .and_then(|e| e.arms.as_ref())
        .map_or(0.45, |a| a.forward);
    preview.1.side = catalog
        .find(&menu.selected)
        .and_then(|e| e.arms.as_ref())
        .map_or(0.27, |a| a.side);
    preview.1.height = catalog
        .find(&menu.selected)
        .and_then(|entry| entry.arms.as_ref())
        .map_or(0.70, |arms| arms.height);
}
