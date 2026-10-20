use bevy::prelude::*;
use boltrunner_client::Session;
use boltrunner_shared::arsenal::WeaponKind;

#[derive(Resource, Default)]
pub(crate) struct WeaponMenu {
    pub open: bool,
    pub selected: WeaponKind,
    pub request_press: u16,
}
#[derive(Component)]
struct MenuRoot;
#[derive(Component)]
struct Choice(WeaponKind);
pub struct WeaponMenuPlugin;
impl Plugin for WeaponMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WeaponMenu>()
            .add_systems(Startup, setup)
            .add_systems(
                PreUpdate,
                hotkeys
                    .in_set(crate::debug_menu::MenuInput)
                    .after(bevy::input::InputSystems),
            )
            .add_systems(Update, interact);
    }
}
fn setup(mut commands: Commands) {
    commands
        .spawn((
            MenuRoot,
            GlobalZIndex(30),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                display: Display::None,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.015, 0.02, 0.035, 0.85)),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: px(980),
                    max_height: percent(94),
                    overflow: Overflow::scroll_y(),
                    max_width: percent(94),
                    padding: UiRect::all(px(22)),
                    row_gap: px(14),
                    flex_direction: FlexDirection::Column,
                    border_radius: BorderRadius::all(px(12)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.045, 0.055, 0.075)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("DEV ARSENAL"),
                    TextFont {
                        font_size: FontSize::Px(26.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.92, 0.95, 1.0)),
                ));
                panel
                    .spawn(Node {
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: px(10),
                        row_gap: px(10),
                        ..default()
                    })
                    .with_children(|grid| {
                        for kind in WeaponKind::ALL {
                            let [r, g, b] = kind.color();
                            grid.spawn((
                                Button,
                                Choice(kind),
                                Node {
                                    width: px(220),
                                    min_height: px(105),
                                    padding: UiRect::all(px(12)),
                                    border: UiRect::all(px(2)),
                                    border_radius: BorderRadius::all(px(7)),
                                    flex_direction: FlexDirection::Column,
                                    row_gap: px(8),
                                    ..default()
                                },
                                BorderColor::all(Color::srgb(r, g, b)),
                                BackgroundColor(Color::srgb(0.07, 0.085, 0.11)),
                            ))
                            .with_children(|button| {
                                button.spawn((
                                    Text::new(format!(
                                        "{}  {}",
                                        [
                                            "1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-",
                                            "=", "Q", "E"
                                        ][kind.index()],
                                        kind.name()
                                    )),
                                    TextFont {
                                        font_size: FontSize::Px(15.0),
                                        ..default()
                                    },
                                    TextColor(Color::srgb(r, g, b)),
                                ));
                                button.spawn((
                                    Text::new(kind.description()),
                                    TextFont {
                                        font_size: FontSize::Px(11.0),
                                        ..default()
                                    },
                                    TextColor(Color::srgb(0.65, 0.71, 0.8)),
                                ));
                            });
                        }
                    });
                panel.spawn((
                    Text::new("1–9 / 0 / - / = / Q / E to grant    F4 / Esc to close"),
                    TextFont {
                        font_size: FontSize::Px(12.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.55, 0.62, 0.72)),
                ));
            });
        });
}
fn hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    session: Res<Session>,
    debug: Res<crate::debug_menu::DebugMenu>,
    mut menu: ResMut<WeaponMenu>,
    maps: Res<crate::map_menu::MapMenu>,
    characters: Res<crate::character_menu::CharacterMenu>,
) {
    if !session.is_playing() {
        menu.request_press = 0;
        menu.selected = WeaponKind::Pistol;
    }
    let previous = menu.open;
    if !session.is_playing()
        || debug.open
        || keys.just_pressed(KeyCode::F2)
        || keys.just_pressed(KeyCode::F1)
        || keys.just_pressed(KeyCode::F3)
    {
        menu.open = false;
    } else if keys.just_pressed(KeyCode::F4) {
        menu.open = !menu.open;
    } else if menu.open {
        if keys.just_pressed(KeyCode::Escape) {
            menu.open = false;
        }
        for (index, key) in [
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
            KeyCode::Digit5,
            KeyCode::Digit6,
            KeyCode::Digit7,
            KeyCode::Digit8,
            KeyCode::Digit9,
            KeyCode::Digit0,
            KeyCode::Minus,
            KeyCode::Equal,
            KeyCode::KeyQ,
            KeyCode::KeyE,
        ]
        .into_iter()
        .enumerate()
        {
            if keys.just_pressed(key) {
                menu.selected = WeaponKind::ALL[index];
                menu.request_press = menu.request_press.wrapping_add(1);
                menu.open = false;
            }
        }
    }
    if previous != menu.open {
        crate::platform::set_debug_menu(menu.open || debug.open || maps.open || characters.open);
    }
}
fn interact(
    mut menu: ResMut<WeaponMenu>,
    mut root: Single<&mut Node, With<MenuRoot>>,
    mut buttons: Query<(&Choice, &Interaction, &mut BackgroundColor)>,
) {
    root.display = if menu.open {
        Display::Flex
    } else {
        Display::None
    };
    let mut picked = None;
    for (choice, interaction, mut color) in &mut buttons {
        let [r, g, b] = choice.0.color();
        color.0 = if choice.0 == menu.selected {
            Color::srgb(r * 0.2, g * 0.2, b * 0.2)
        } else if *interaction == Interaction::Hovered {
            Color::srgb(0.13, 0.16, 0.21)
        } else {
            Color::srgb(0.07, 0.085, 0.11)
        };
        if menu.open && *interaction == Interaction::Pressed {
            picked = Some(choice.0);
        }
    }
    if let Some(kind) = picked {
        menu.selected = kind;
        menu.request_press = menu.request_press.wrapping_add(1);
        menu.open = false;
        root.display = Display::None;
        crate::platform::set_debug_menu(false);
    }
}
