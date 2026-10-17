use bevy::prelude::*;
use boltrunner_shared::{crystals::Crystal, level};
use lightyear::prelude::*;

pub fn maintain(
    mut commands: Commands,
    round: Res<crate::matches::CurrentMatch>,
    mut revision: Local<Option<u64>>,
    mut crystals: Query<&mut Crystal>,
) {
    if *revision != Some(round.map_revision) {
        for (slot, spawn) in level::data().crystals.iter().enumerate() {
            commands.spawn((
                Crystal::new(slot as u16, spawn),
                Replicate::to_clients(NetworkTarget::All),
            ));
        }
        *revision = Some(round.map_revision);
        return;
    }
    for mut crystal in &mut crystals {
        if !crystal.active {
            crystal.advance();
        }
    }
}
