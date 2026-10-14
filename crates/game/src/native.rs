use bevy::{
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PresentMode, PrimaryWindow, WindowResolution},
};
use boltrunner_client::ServerUrl;

const HELP: &str = "Boltrunner native client

Usage: boltrunner [OPTIONS]
  --server URL       WebSocket server (default ws://127.0.0.1:5000)
  --width PIXELS     Requested physical window width (default 1280)
  --height PIXELS    Requested physical window height (default 720)
  --uncapped         Disable VSync (default: VSync on)
  --help             Show this help

Click to capture mouse; Esc releases it. WASD, Space, Shift, left mouse to fire.
";

struct Options {
    server: String,
    width: u32,
    height: u32,
    uncapped: bool,
}

impl Options {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Option<Self>, String> {
        let mut options = Self {
            server: "ws://127.0.0.1:5000".into(),
            width: 1280,
            height: 720,
            uncapped: false,
        };
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--help" | "-h" => return Ok(None),
                "--uncapped" => options.uncapped = true,
                "--server" | "--width" | "--height" => {
                    let value = args
                        .next()
                        .ok_or_else(|| format!("{arg} requires a value"))?;
                    if arg == "--server" {
                        if !value.starts_with("ws://") && !value.starts_with("wss://") {
                            return Err("--server must start with ws:// or wss://".into());
                        }
                        options.server = value;
                    } else {
                        let pixels = value
                            .parse::<u32>()
                            .ok()
                            .filter(|n| (64..=16384).contains(n))
                            .ok_or_else(|| format!("{arg} must be between 64 and 16384 pixels"))?;
                        if arg == "--width" {
                            options.width = pixels;
                        } else {
                            options.height = pixels;
                        }
                    }
                }
                _ => return Err(format!("unknown option: {arg}")),
            }
        }
        Ok(Some(options))
    }
}

pub fn configure(app: &mut App) {
    let options = match Options::parse(std::env::args().skip(1)) {
        Ok(Some(options)) => options,
        Ok(None) => {
            print!("{HELP}");
            std::process::exit(0);
        }
        Err(error) => {
            eprintln!("{error}\n\n{HELP}");
            std::process::exit(2);
        }
    };
    app.insert_resource(ServerUrl(options.server))
        .add_plugins(
            DefaultPlugins
                .set(bevy::asset::io::web::WebAssetPlugin {
                    silence_startup_warning: true,
                })
                .set(AssetPlugin {
                    file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets").into(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Boltrunner — native".into(),
                        resolution: WindowResolution::new(options.width, options.height)
                            .with_scale_factor_override(1.0),
                        present_mode: if options.uncapped {
                            PresentMode::AutoNoVsync
                        } else {
                            PresentMode::AutoVsync
                        },
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_systems(
            PreUpdate,
            capture_pointer
                .after(bevy::input::InputSystems)
                .after(crate::debug_menu::MenuInput)
                .after(crate::character_menu::hotkeys)
                .before(crate::view::look_input),
        );
}

fn capture_pointer(
    session: Res<boltrunner_client::Session>,
    settings: Res<crate::debug_menu::DebugMenu>,
    maps: Res<crate::map_menu::MapMenu>,
    weapons: Res<crate::weapon_menu::WeaponMenu>,
    characters: Res<crate::character_menu::CharacterMenu>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut was_open: Local<bool>,
    mut window: Single<(&Window, &mut CursorOptions), With<PrimaryWindow>>,
) {
    if !session.is_playing()
        || settings.open
        || maps.open
        || weapons.open
        || characters.open
        || !window.0.focused
        || keys.just_pressed(KeyCode::Escape)
    {
        window.1.grab_mode = CursorGrabMode::None;
        window.1.visible = true;
    } else if !*was_open && buttons.just_pressed(MouseButton::Left) {
        window.1.grab_mode = CursorGrabMode::Locked;
        window.1.visible = false;
    }
    *was_open = settings.open || weapons.open || characters.open;
}

pub fn pointer_locked(cursor: &CursorOptions) -> bool {
    cursor.grab_mode != CursorGrabMode::None
}

pub fn finished_loading() {
    info!("{} loaded.", boltrunner_shared::level::data().name);
}

// The shared Bevy loading screen presents progress on native platforms.
pub fn show_loading() {}

pub fn set_playing(_playing: bool) {}

pub fn set_debug_menu(_open: bool) {}
