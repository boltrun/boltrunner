use bevy::prelude::*;
use boltrunner_shared::{
    PlayerId, PlayerState, level,
    powerups::{Pickup, PickupKind},
    tuning::{MatchTuning, seconds_to_ticks},
};
use lightyear::prelude::*;

#[derive(Resource, Default)]
pub struct PickupDirector {
    initialized: bool,
    remaining_ticks: u16,
}

pub fn collect(
    mut commands: Commands,
    round: Res<crate::matches::CurrentMatch>,
    settings: Res<crate::tuning::ServerSettings>,
    pickups: Query<(Entity, &Pickup)>,
    mut players: Query<(&PlayerId, &mut PlayerState)>,
) {
    if round.results {
        return;
    }
    for (entity, pickup) in &pickups {
        for (id, mut player) in &mut players {
            let center = player.position + Vec3::Y * (player.tuning.player_height / 2.0);
            if center.distance_squared(pickup.position)
                > settings.values.rules.pickup_radius.powi(2)
            {
                continue;
            }
            // A nearby pickup on the far side of a thin wall is not reachable.
            if level::world()
                .sweep_sphere(center, pickup.position - center, 0.02)
                .is_some()
            {
                continue;
            }
            if pickup.apply(&mut player) {
                commands.entity(entity).despawn();
                info!("Player {:016x} collected {:?}", id.0, pickup.kind);
                break;
            }
        }
    }
}

pub fn manage(
    mut commands: Commands,
    mut director: ResMut<PickupDirector>,
    round: Res<crate::matches::CurrentMatch>,
    settings: Res<crate::tuning::ServerSettings>,
    pickups: Query<(Entity, &Pickup)>,
    players: Query<&PlayerState>,
) {
    if round.results {
        for (entity, _) in &pickups {
            commands.entity(entity).despawn();
        }
        director.initialized = false;
        director.remaining_ticks = 0;
        return;
    }

    let rules = settings.values.rules;
    let limit = rules.active_pickups as usize;
    let mut occupied: Vec<Vec3> = pickups.iter().map(|(_, pickup)| pickup.position).collect();
    if occupied.len() > limit {
        for (entity, _) in pickups.iter().skip(limit) {
            commands.entity(entity).despawn();
        }
        occupied.truncate(limit);
    }
    if !director.initialized {
        let initial = [
            PickupKind::Health,
            PickupKind::Speed,
            PickupKind::RapidFire,
            PickupKind::Health,
            PickupKind::Speed,
            PickupKind::RapidFire,
        ];
        for index in 0..limit {
            spawn_random(
                &mut commands,
                initial[index % initial.len()],
                &mut occupied,
                &players,
                rules,
            );
        }
        info!("Spawned {} map powerups", occupied.len());
        director.initialized = true;
        director.remaining_ticks = seconds_to_ticks(rules.pickup_respawn);
        return;
    }
    director.remaining_ticks = director.remaining_ticks.saturating_sub(1);
    if director.remaining_ticks == 0 {
        if occupied.len() < limit {
            let kind = match rand::random::<u8>() % 3 {
                0 => PickupKind::Health,
                1 => PickupKind::Speed,
                _ => PickupKind::RapidFire,
            };
            spawn_random(&mut commands, kind, &mut occupied, &players, rules);
        }
        director.remaining_ticks = seconds_to_ticks(rules.pickup_respawn);
    }
}

fn spawn_random(
    commands: &mut Commands,
    kind: PickupKind,
    occupied: &mut Vec<Vec3>,
    players: &Query<&PlayerState>,
    rules: MatchTuning,
) {
    let candidates = level::pickup_slots();
    if candidates.is_empty() {
        return;
    }
    let start = rand::random::<u32>() as usize % candidates.len();
    for offset in 0..candidates.len() {
        let feet = candidates[(start + offset) % candidates.len()];
        let position = feet + Vec3::Y * 0.65;
        if occupied
            .iter()
            .any(|other| other.distance_squared(position) < rules.pickup_spacing.max(0.01).powi(2))
            || players.iter().any(|player| {
                player.position.distance_squared(feet) < rules.pickup_player_distance.powi(2)
            })
        {
            continue;
        }
        commands.spawn((
            Pickup { kind, position },
            Replicate::to_clients(NetworkTarget::All),
        ));
        occupied.push(position);
        return;
    }
}
