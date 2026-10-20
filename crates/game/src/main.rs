mod arsenal;
mod baked_material;
mod character_menu;
mod characters;
mod debug_menu;
#[cfg(target_arch = "wasm32")]
mod direct_output;
mod hud;
mod hud_font;
mod inventory;
mod loading;
mod map;
mod map_decor;
mod map_menu;
mod mipmaps;
mod nameplates;
mod powerups;
mod projectile_visual;
mod reload_animation;
mod title;
mod view;
mod weapon;
mod weapon_menu;
mod weapon_visual;
mod world_weapon;
mod zipline;

#[cfg(not(target_arch = "wasm32"))]
#[path = "native.rs"]
mod platform;
#[cfg(target_arch = "wasm32")]
#[path = "web.rs"]
mod platform;

use bevy::{prelude::*, winit::WinitSettings};
use boltrunner_client::GameNetworkingPlugin;

fn main() -> AppExit {
    let mut app = App::new();
    platform::configure(&mut app);
    #[cfg(target_arch = "wasm32")]
    direct_output::install(&mut app);
    // Bundle the full font for identical Unicode coverage on web and native.
    app.world_mut()
        .resource_mut::<Assets<Font>>()
        .insert(
            bevy::asset::AssetId::default(),
            Font::from_bytes(include_bytes!("../../../assets/fonts/DejaVuSansMono.ttf").to_vec()),
        )
        .unwrap();
    app.insert_resource(WinitSettings::continuous())
        .add_plugins(world_weapon::WorldWeaponsPlugin)
        .add_plugins(map_menu::MapMenuPlugin)
        .add_plugins(map_decor::MapDecorPlugin)
        .add_plugins(inventory::InventoryVisualsPlugin)
        .add_plugins(zipline::ZiplineVisualPlugin)
        .add_plugins(projectile_visual::ProjectileVisualPlugin)
        .add_plugins((
            GameNetworkingPlugin,
            characters::CharactersPlugin,
            character_menu::CharacterMenuPlugin,
            loading::LoadingPlugin,
            title::TitlePlugin,
            hud::HudPlugin,
            MaterialPlugin::<baked_material::BakedMaterial>::default(),
            view::ViewPlugin,
            debug_menu::DebugMenuPlugin,
            nameplates::NameplatesPlugin,
            powerups::PowerupVisualsPlugin,
            weapon::WeaponPlugin,
            weapon_menu::WeaponMenuPlugin,
            arsenal::ArsenalVisualsPlugin,
        ))
        .run()
}
