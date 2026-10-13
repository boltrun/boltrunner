use bevy::{
    color::Mix,
    input::mouse::{MouseScrollUnit, MouseWheel},
    prelude::*,
    window::PrimaryWindow,
};
use boltrunner_client::{Session, maps::MapSelection};
use boltrunner_shared::level;

#[derive(Resource, Default)]
pub(crate) struct MapMenu {
    pub open: bool,
}
#[derive(Component)]
struct Panel;
#[derive(Component)]
struct WindowPanel;
#[derive(Component)]
struct Listing;
#[derive(Component)]
struct Status;
#[derive(Component)]
enum Action {
    Close,
    Select(&'static str),
}
#[derive(Component)]
struct MapLabel(&'static str);

pub struct MapMenuPlugin;
impl Plugin for MapMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapMenu>()
            .add_systems(Startup, setup)
            .add_systems(
                PreUpdate,
                hotkeys
                    .in_set(crate::debug_menu::MenuInput)
                    .after(crate::debug_menu::hotkeys),
            )
            .add_systems(
                Update,
                (interact, present, fit, scroll)
                    .chain()
                    .after(boltrunner_client::update_session),
            );
    }
}
fn label(parent: &mut ChildSpawnerCommands, value: &str) {
    parent.spawn((
        Text::new(value),
        crate::debug_menu::menu_font(),
        TextColor(Color::srgb(0.82, 0.82, 0.82)),
    ));
}
fn setup(mut commands: Commands) {
    commands
        .spawn((
            Panel,
            GlobalZIndex(1000),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.48)),
        ))
        .with_children(|root| {
            root.spawn((
                WindowPanel,
                Node {
                    width: px(600),
                    height: px(600),
                    padding: UiRect::all(px(16)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(12),
                    border_radius: BorderRadius::all(px(8)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.10, 0.10, 0.10)),
            ))
            .with_children(|panel| {
                panel
                    .spawn(Node {
                        align_items: AlignItems::Center,
                        column_gap: px(6),
                        flex_shrink: 0.0,
                        ..default()
                    })
                    .with_children(|top| {
                        top.spawn(Node {
                            flex_grow: 1.0,
                            ..default()
                        })
                        .with_children(|p| label(p, "Maps"));
                        top.spawn((
                            Button,
                            Action::Close,
                            Node {
                                width: px(32),
                                min_height: px(32),
                                flex_shrink: 0.0,
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                padding: UiRect::horizontal(px(8)),
                                border_radius: BorderRadius::all(px(6)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.10, 0.10, 0.10)),
                        ))
                        .with_children(|p| label(p, "×"));
                    });
                panel
                    .spawn((
                        Listing,
                        ScrollPosition::default(),
                        Node {
                            flex_grow: 1.0,
                            flex_shrink: 1.0,
                            min_height: px(0),
                            flex_direction: FlexDirection::Column,
                            overflow: Overflow::scroll_y(),
                            row_gap: px(2),
                            ..default()
                        },
                    ))
                    .with_children(|listing| {
                        for map in &level::MAPS {
                            listing
                                .spawn((
                                    Button,
                                    Action::Select(map.id),
                                    Node {
                                        min_height: px(32),
                                        flex_shrink: 0.0,
                                        align_items: AlignItems::Center,
                                        padding: UiRect::horizontal(px(8)),
                                        border_radius: BorderRadius::all(px(6)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(0.10, 0.10, 0.10)),
                                ))
                                .with_children(|p| {
                                    p.spawn((
                                        MapLabel(map.id),
                                        Text::new(&map.data().name),
                                        crate::debug_menu::menu_font(),
                                        TextColor(Color::srgb(0.82, 0.82, 0.82)),
                                    ));
                                });
                        }
                    });
                panel.spawn((
                    Status,
                    Text::new(""),
                    crate::debug_menu::menu_font(),
                    TextColor(Color::srgb(0.82, 0.82, 0.82)),
                    Node {
                        flex_shrink: 0.0,
                        ..default()
                    },
                ));
            });
        });
}
pub(crate) fn hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    session: Res<Session>,
    mut menu: ResMut<MapMenu>,
    mut debug: ResMut<crate::debug_menu::DebugMenu>,
    characters: Res<crate::character_menu::CharacterMenu>,
    mut previous: Local<bool>,
) {
    if !session.is_playing()
        || keys.just_pressed(KeyCode::F1)
        || keys.just_pressed(KeyCode::F2)
        || keys.just_pressed(KeyCode::Escape)
    {
        menu.open = false;
    } else if keys.just_pressed(KeyCode::F3) {
        menu.open = !menu.open;
        if menu.open {
            debug.open = false;
        }
    }
    let open = menu.open || debug.open || characters.open;
    if open != *previous {
        crate::platform::set_debug_menu(open);
        *previous = open;
    }
}
fn interact(
    session: Res<Session>,
    mut menu: ResMut<MapMenu>,
    debug: Res<crate::debug_menu::DebugMenu>,
    mut selection: ResMut<MapSelection>,
    actions: Query<(&Interaction, &Action), Changed<Interaction>>,
) {
    if !session.is_playing() {
        return;
    }
    for (interaction, action) in &actions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            Action::Close => menu.open = false,
            Action::Select(id) if menu.open && session.map.as_deref() != Some(id) => {
                selection.choose(id)
            }
            _ => (),
        }
        crate::platform::set_debug_menu(menu.open || debug.open);
    }
}
fn present(
    menu: Res<MapMenu>,
    session: Res<Session>,
    selection: Res<MapSelection>,
    time: Res<Time>,
    mut panels: Query<&mut Node, With<Panel>>,
    mut status: Query<(&mut Text, &mut TextColor), (With<Status>, Without<MapLabel>)>,
    mut labels: Query<(&MapLabel, &mut Text), Without<Status>>,
    mut buttons: Query<(&Action, &Interaction, &mut BackgroundColor)>,
) {
    for mut node in &mut panels {
        node.display = if menu.open && session.is_playing() {
            Display::Flex
        } else {
            Display::None
        };
    }
    for (mut text, mut color) in &mut status {
        color.0 = if selection.error.is_some() {
            Color::srgb(0.9, 0.68, 0.68)
        } else {
            Color::srgb(0.82, 0.82, 0.82)
        };
        text.0 = selection.error.clone().unwrap_or_else(|| {
            if selection.pending {
                "Changing map…".into()
            } else {
                String::new()
            }
        });
    }
    for (label, mut text) in &mut labels {
        let map = level::MAPS.iter().find(|m| m.id == label.0).unwrap();
        text.0 = format!(
            "{}{}",
            map.data().name,
            if session.map.as_deref() == Some(label.0) {
                " — current"
            } else {
                " — switch"
            }
        );
    }
    let blend = 1.0 - (-18.0 * time.delta_secs()).exp();
    for (action, interaction, mut color) in &mut buttons {
        let selected = matches!(action, Action::Select(id) if session.map.as_deref() == Some(id));
        let pending = selection.pending && matches!(action, Action::Select(_));
        let value = if pending {
            0.075
        } else {
            match interaction {
                Interaction::Pressed => 0.25,
                Interaction::Hovered if selected => 0.25,
                _ if selected => 0.21,
                Interaction::Hovered => 0.17,
                _ => 0.10,
            }
        };
        color.0 = color.0.mix(&Color::srgb(value, value, value), blend);
    }
}
fn fit(
    window: Single<&Window, With<PrimaryWindow>>,
    mut panel: Single<&mut Node, With<WindowPanel>>,
) {
    let side = (window.width().min(window.height()) - 24.0).clamp(100.0, 600.0);
    panel.width = px(side);
    panel.height = px(side);
}
fn scroll(
    menu: Res<MapMenu>,
    mut events: MessageReader<MouseWheel>,
    mut listing: Single<(&ComputedNode, &mut ScrollPosition), With<Listing>>,
) {
    if !menu.open {
        events.clear();
        return;
    }
    for event in events.read() {
        let scale = match event.unit {
            MouseScrollUnit::Line => 32.0,
            MouseScrollUnit::Pixel => 1.0,
        };
        let max = ((listing.0.content_size().y - listing.0.size().y)
            * listing.0.inverse_scale_factor())
        .max(0.0);
        listing.1.y = (listing.1.y - event.y * scale).clamp(0.0, max);
    }
}
