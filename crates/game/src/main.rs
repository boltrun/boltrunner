mod arsenal;
mod baked_material;
mod debug_menu;
mod inventory;
mod kill_feed;
mod loading;
mod map;
mod map_menu;
mod mipmaps;
mod nameplates;
mod powerups;
mod projectile_visual;
mod reload_animation;
mod scoreboard;
mod title;
mod view;
mod weapon;
mod weapon_menu;
mod weapon_visual;
mod world_weapon;

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
    // Bundle the full font for identical Unicode coverage on web and native.
    app.world_mut()
        .resource_mut::<Assets<Font>>()
        .insert(
            bevy::asset::AssetId::default(),
            Font::try_from_bytes(
                include_bytes!("../../../assets/fonts/DejaVuSansMono.ttf").to_vec(),
            )
            .expect("bundled UI font must be valid"),
        )
        .unwrap();
    app.insert_resource(WinitSettings::continuous())
        .add_plugins(world_weapon::WorldWeaponsPlugin)
        .add_plugins(inventory::InventoryVisualsPlugin)
        .add_plugins(projectile_visual::ProjectileVisualPlugin)
        .add_plugins((
            GameNetworkingPlugin,
            loading::LoadingPlugin,
            title::TitlePlugin,
            scoreboard::ScoreboardPlugin,
            kill_feed::KillFeedPlugin,
            MaterialPlugin::<baked_material::BakedMaterial>::default(),
            view::ViewPlugin,
            debug_menu::DebugMenuPlugin,
            map_menu::MapMenuPlugin,
            nameplates::NameplatesPlugin,
            powerups::PowerupVisualsPlugin,
            weapon::WeaponPlugin,
            weapon_menu::WeaponMenuPlugin,
            arsenal::ArsenalVisualsPlugin,
        ))
        .run()
}
