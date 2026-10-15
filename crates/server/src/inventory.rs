use bevy::prelude::*;
use boltrunner_shared::{
    PlayerInput, PlayerState, TICK_DURATION,
    inventory::{GroundWeapon, HeldWeapon, reachable},
    level,
    match_state::MatchState,
    tuning::FeatureSettings,
};
use lightyear::prelude::{input::native::ActionState, *};
use std::collections::HashSet;

#[derive(Component)]
pub struct FallingWeapon(Vec3);

pub fn drop_weapon(commands: &mut Commands, weapon: HeldWeapon, state: &PlayerState, side: f32) {
    let center = state.position + Vec3::Y * 0.8;
    let offset = Quat::from_rotation_y(state.yaw) * Vec3::new(side, 0.1, 0.0);
    let position = level::world().clip_camera(center, center + offset, 0.2);
    commands.spawn((
        GroundWeapon {
            weapon,
            position,
            map_spawn: false,
        },
        FallingWeapon(Vec3::ZERO),
        Replicate::to_clients(NetworkTarget::All),
    ));
}

/// Run before the shared input step, which consumes the slot press counter.
pub fn collect(
    mut commands: Commands,
    round: Res<MatchState>,
    settings: Res<FeatureSettings>,
    weapons: Query<(Entity, &GroundWeapon)>,
    mut players: Query<(&mut PlayerState, &ActionState<PlayerInput>)>,
) {
    if round.results {
        return;
    }
    let mut claimed = HashSet::new();
    for (mut state, input) in &mut players {
        if input.0.slot_press == state.weapon.last_slot_press || !(1..=2).contains(&input.0.slot) {
            continue;
        }
        let picked = weapons
            .iter()
            .filter(|(entity, item)| {
                !claimed.contains(entity)
                    && !boltrunner_shared::inventory::owns(&state.weapon, item.weapon.kind)
                    && reachable(&state, item.position, settings.values.rules.pickup_radius)
            })
            .min_by(|(a, wa), (b, wb)| {
                wa.position
                    .distance_squared(state.position)
                    .total_cmp(&wb.position.distance_squared(state.position))
                    .then(a.to_bits().cmp(&b.to_bits()))
            })
            .map(|(entity, item)| (entity, *item));
        let Some((entity, item)) = picked else {
            continue;
        };
        claimed.insert(entity);
        let slot = input.0.slot as usize - 1;
        if let Some(previous) = state.weapon.slots[slot].replace(item.weapon) {
            drop_weapon(&mut commands, previous, &state, 0.65);
        }
        let tuning = state.tuning;
        state.weapon.select_slot(input.0.slot, tuning);
        if let Some(drone) = &mut state.drone {
            drone.controlling = false;
        }
        commands.entity(entity).despawn();
    }
}

pub fn settle(
    settings: Res<FeatureSettings>,
    mut weapons: Query<(&mut GroundWeapon, Option<&mut FallingWeapon>)>,
) {
    let dt = TICK_DURATION.as_secs_f32();
    for (mut item, velocity) in &mut weapons {
        if item.map_spawn {
            item.weapon = HeldWeapon::full(item.weapon.kind, settings.values.simulation);
        }
        let Some(mut velocity) = velocity else {
            continue;
        };
        velocity.0.y -= 24.0 * dt;
        let delta = velocity.0 * dt;
        if let Some(hit) = level::world().sweep_sphere(item.position, delta, 0.2) {
            item.position += delta * hit;
            velocity.0 = Vec3::ZERO;
        } else {
            item.position += delta;
        }
    }
}
