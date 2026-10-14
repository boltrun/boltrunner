//! Replicated cosmetic equipment shared by character and arsenal presentation.
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerEquipment {
    pub weapon: u8,
    pub pitch: i16,
}

pub const WEAPON_MODELS: [&str; 14] = [
    "weapons/starter_pistol/built/starter_pistol.glb",
    "weapons/arsenal/built/flamethrower.gltf",
    "weapons/arsenal/built/shotgun.gltf",
    "weapons/arsenal/built/machine_gun.gltf",
    "weapons/arsenal/built/rocket_launcher.gltf",
    "weapons/arsenal/built/laser.gltf",
    "weapons/arsenal/built/sniper.gltf",
    "weapons/arsenal/built/drone_controller.gltf",
    "weapons/arsenal/built/ricochet.gltf",
    "weapons/arsenal/built/karambit.gltf",
    "weapons/arsenal/built/arc_cannon.gltf",
    "weapons/arsenal/built/plasma_launcher.gltf",
    "weapons/arsenal/built/electro_plasma.gltf",
    "weapons/arsenal/built/seeker_plasma.gltf",
];
