use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{PlayerState, tuning::seconds_to_ticks};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub enum PickupKind {
    Health,
    Speed,
    RapidFire,
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Reflect)]
pub struct Pickup {
    pub kind: PickupKind,
    pub position: Vec3,
}

impl Pickup {
    pub fn apply(self, player: &mut PlayerState) -> bool {
        if player.death.is_some() || player.match_paused {
            return false;
        }
        match self.kind {
            PickupKind::Health => {
                let amount = (player.tuning.max_health as f32
                    * player.tuning.health_pickup_fraction)
                    .round() as u16;
                player.health.0 = player
                    .health
                    .0
                    .saturating_add(amount)
                    .min(player.tuning.max_health as u16);
                true
            }
            PickupKind::Speed => {
                player.speed_ticks = seconds_to_ticks(player.tuning.speed_duration);
                true
            }
            PickupKind::RapidFire => {
                player.rapid_fire_ticks = seconds_to_ticks(player.tuning.rapid_fire_duration);
                let kind = player.weapon.selected;
                player.weapon.cooldowns[kind.index()] =
                    player.weapon.cooldowns[kind.index()].min(seconds_to_ticks(
                        kind.stats(player.tuning).interval / player.tuning.rapid_fire_multiplier,
                    ));
                true
            }
        }
    }
}
