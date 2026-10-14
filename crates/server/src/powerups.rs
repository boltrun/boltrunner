use bevy::prelude::*;
use boltrunner_shared::{
    PlayerId, PlayerState, level,
    powerups::{Pickup, PickupKind},
    tuning::{FeatureSettings, seconds_to_ticks},
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
    weapons: Query<(Entity, &boltrunner_shared::inventory::GroundWeapon)>,
    fields: Query<Entity, With<boltrunner_shared::weapon::ShockField>>,
    players: Query<&PlayerState>,
) {
    if round.results {
        for (entity, _) in &pickups {
            commands.entity(entity).despawn();
        }
        for (entity, _) in &weapons {
            commands.entity(entity).despawn();
        }
        for entity in &fields {
            commands.entity(entity).despawn();
        }
        director.initialized = false;
        director.remaining_ticks = 0;
        return;
    }
    let rules = settings.values.rules;
    let limit = rules.active_pickups as usize;
    let managed: Vec<_> = pickups
        .iter()
        .map(|(e, p)| (e, p.position))
        .chain(
            weapons
                .iter()
                .filter(|(_, w)| w.map_spawn)
                .map(|(e, w)| (e, w.position)),
        )
        .collect();
    for (entity, _) in managed.iter().skip(limit) {
        commands.entity(*entity).despawn();
    }
    let mut current = managed.len().min(limit);
    let mut occupied: Vec<Vec3> = managed
        .iter()
        .take(limit)
        .map(|(_, p)| *p)
        .chain(
            weapons
                .iter()
                .filter(|(_, w)| !w.map_spawn)
                .map(|(_, w)| w.position),
        )
        .collect();
    if !director.initialized {
        for index in current..limit {
            let kind = [PickupKind::Health, PickupKind::Speed, PickupKind::RapidFire][index % 3];
            if spawn_random(&mut commands, kind, &mut occupied, &players, &settings) {
                current += 1;
            }
        }
        info!("Spawned {} map pickups (powerups and weapons)", current);
        director.initialized = true;
        director.remaining_ticks = seconds_to_ticks(rules.pickup_respawn);
    } else {
        director.remaining_ticks = director.remaining_ticks.saturating_sub(1);
        if director.remaining_ticks == 0 {
            if current < limit {
                let kind = [PickupKind::Health, PickupKind::Speed, PickupKind::RapidFire]
                    [rand::random::<u32>() as usize % 3];
                spawn_random(&mut commands, kind, &mut occupied, &players, &settings);
            }
            director.remaining_ticks = seconds_to_ticks(rules.pickup_respawn);
        }
    }
}

fn spawn_random(
    commands: &mut Commands,
    kind: PickupKind,
    occupied: &mut Vec<Vec3>,
    players: &Query<&PlayerState>,
    settings: &FeatureSettings,
) -> bool {
    let rules = settings.values.rules;
    let candidates = level::pickup_slots();
    if candidates.is_empty() {
        return false;
    }
    let start = rand::random::<u32>() as usize % candidates.len();
    for offset in 0..candidates.len() {
        let position = candidates[(start + offset) % candidates.len()] + Vec3::Y * 0.65;
        if occupied
            .iter()
            .any(|other| other.distance_squared(position) < rules.pickup_spacing.max(0.01).powi(2))
            || players.iter().any(|p| {
                p.death.is_none()
                    && p.position.distance_squared(position - Vec3::Y * 0.65)
                        < rules.pickup_player_distance.powi(2)
            })
        {
            continue;
        }
        if rand::random::<f32>() * 100.0 < rules.weapon_spawn_chance {
            use boltrunner_shared::{
                arsenal::WeaponKind,
                inventory::{GroundWeapon, HeldWeapon},
            };
            let kind =
                WeaponKind::ALL[1 + rand::random::<u32>() as usize % (WeaponKind::ALL.len() - 1)];
            commands.spawn((
                GroundWeapon {
                    weapon: HeldWeapon::full(kind, settings.values.simulation),
                    position,
                    map_spawn: true,
                },
                Replicate::to_clients(NetworkTarget::All),
            ));
        } else {
            commands.spawn((
                Pickup { kind, position },
                Replicate::to_clients(NetworkTarget::All),
            ));
        }
        occupied.push(position);
        return true;
    }
    false
}
