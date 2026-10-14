use bevy::prelude::*;
use boltrunner_shared::{PlayerInput, equipment::PlayerEquipment};
use lightyear::prelude::{ReplicationSystems, input::native::ActionState};

pub struct EquipmentPlugin;

impl Plugin for EquipmentPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, publish.before(ReplicationSystems::Send));
    }
}

fn publish(mut players: Query<(&ActionState<PlayerInput>, &mut PlayerEquipment)>) {
    for (input, mut equipment) in &mut players {
        let value = PlayerEquipment {
            weapon: equipment.weapon,
            pitch: input.0.pitch,
        };
        if *equipment != value {
            *equipment = value;
        }
    }
}
