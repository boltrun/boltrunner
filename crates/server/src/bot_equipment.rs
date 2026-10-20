use crate::bots::{BotContact, BotLootIntent, BotLootTarget, BotLootTargets, BotSystems};
use bevy::prelude::*;
use boltrunner_shared::{
    PlayerInput, PlayerState,
    arsenal::WeaponKind,
    inventory::{GroundWeapon, HeldWeapon, reachable},
    protocol::BotPlayer,
    tuning::SimulationTuning,
};
use lightyear::prelude::input::native::ActionState;
use std::collections::{HashMap, VecDeque};

pub struct BotEquipmentPlugin;
impl Plugin for BotEquipmentPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EquipmentDirector>()
            .add_observer(track_weapon)
            .add_systems(
                FixedUpdate,
                offer_weapons
                    .after(BotSystems::Decide)
                    .before(BotSystems::Equipment),
            )
            .add_systems(FixedUpdate, control.in_set(BotSystems::Equipment));
    }
}
#[derive(Resource, Default)]
struct EquipmentDirector {
    tick: u64,
    map: String,
    plans: HashMap<Entity, WeaponPlan>,
    weapons: VecDeque<Entity>,
}
#[derive(Clone, Copy)]
struct WeaponPlan {
    slot: u8,
}

fn track_weapon(event: On<Add, GroundWeapon>, mut director: ResMut<EquipmentDirector>) {
    director.weapons.push_front(event.entity);
}

fn power(weapon: HeldWeapon, tuning: SimulationTuning, distance: f32) -> f32 {
    let kind = weapon.kind;
    let stats = kind.stats(tuning);
    if matches!(
        kind,
        WeaponKind::Flame
            | WeaponKind::Karambit
            | WeaponKind::Arc
            | WeaponKind::Laser
            | WeaponKind::Sniper
    ) && distance > stats.range + tuning.player_radius
    {
        return 0.0;
    }
    if kind.is_projectile() && distance > stats.speed * stats.lifetime {
        return 0.0;
    }
    if stats.blast_radius > 0.0 && distance < stats.blast_radius + tuning.player_radius + 1.0 {
        return 0.0;
    }
    if kind == WeaponKind::Drone {
        return tuning.drone_damage as f32 / (1.0 + distance / tuning.drone_speed.max(0.1)) + 15.0;
    }
    let spread = if kind == WeaponKind::MachineGun {
        (stats.spread + tuning.mg_max_spread) * 0.5
    } else {
        stats.spread
    };
    let accuracy = if kind == WeaponKind::Flame || kind == WeaponKind::Karambit {
        1.0
    } else {
        ((tuning.player_radius + stats.radius) / (distance * spread.to_radians().tan()).max(0.001))
            .min(1.0)
    };
    let fade = ((distance - stats.falloff_start)
        / (stats.falloff_end - stats.falloff_start).max(0.001))
    .clamp(0.0, 1.0);
    let hit =
        stats.damage * stats.pellets as f32 * (1.0 - fade * (1.0 - stats.min_damage)) * accuracy
            + stats.blast_damage * 0.8;
    let interval = stats.interval.max(0.05);
    let magazine_time = kind.ammo_limit(tuning).map_or(1000.0, |limit| {
        if kind.is_stream() {
            limit as f32 * boltrunner_shared::TICK_DURATION.as_secs_f32()
        } else {
            limit as f32 * interval
        }
    });
    let reload = kind.reload_seconds(tuning)
        * if kind.loads_individually() {
            kind.ammo_limit(tuning).unwrap_or(1) as f32
        } else {
            1.0
        };
    let uptime = magazine_time / (magazine_time + reload).max(0.001);
    let flight = if kind.is_projectile() {
        distance / stats.speed.max(1.0)
    } else {
        0.0
    };
    let burst = if kind.is_stream() {
        hit / interval * 0.25
    } else {
        hit * 0.5
    };
    let mut value = (hit / interval * uptime + burst) / (1.0 + flight * 0.35);
    if kind == WeaponKind::Laser {
        value /= 1.0 + tuning.laser_charge;
    }
    if kind == WeaponKind::Flame {
        value += tuning.flame_burn_dps;
    }
    // Empty firearms can be reloaded from infinite reserves. Keep their
    // potential utility, discounted for the wait, instead of abandoning them.
    value * if weapon.usable(tuning) { 1.0 } else { 0.8 }
}

// Return the slot that improves the loadout without throwing away a useful gun.
fn pickup_slot(state: &PlayerState, item: HeldWeapon) -> Option<u8> {
    if item.kind == WeaponKind::Pistol {
        return None;
    }
    if state
        .weapon
        .slots
        .iter()
        .flatten()
        .any(|w| w.kind == item.kind)
    {
        return None;
    }
    if let Some(empty) = state.weapon.slots.iter().position(|w| w.is_none()) {
        return Some(empty as u8 + 1);
    }
    let weak = (0..2).min_by(|&a, &b| {
        power(state.weapon.slots[a].unwrap(), state.tuning, 10.0).total_cmp(&power(
            state.weapon.slots[b].unwrap(),
            state.tuning,
            10.0,
        ))
    })?;
    let previous = state.weapon.slots[weak].unwrap();
    (power(item, state.tuning, 10.0) > power(previous, state.tuning, 10.0) * 1.35)
        .then_some(weak as u8 + 1)
}

fn offer_weapons(
    mut director: ResMut<EquipmentDirector>,
    mut offers: ResMut<BotLootTargets>,
    round: Res<crate::matches::CurrentMatch>,
    weapons: Query<(Entity, &GroundWeapon)>,
    bots: Query<(Entity, &PlayerState), With<BotPlayer>>,
    contacts: Query<&BotContact>,
) {
    director.tick = director.tick.wrapping_add(1);
    let changed = director.map != round.map;
    if !changed && director.tick % 15 != 1 {
        return;
    }
    director.map.clone_from(&round.map);
    offers.0.clear();
    director.plans.clear();
    if round.results {
        return;
    }
    // Rotate a bounded observer-fed queue. This does not walk past thousands
    // of old death drops just to get to this tick's batch.
    let mut items = Vec::new();
    for _ in 0..director.weapons.len().min(128) {
        let target = director.weapons.pop_front().unwrap();
        if let Ok((_, item)) = weapons.get(target) {
            items.push((target, *item));
            director.weapons.push_back(target);
        }
    }
    for (bot, state) in &bots {
        if state.death.is_some() || state.match_paused {
            continue;
        }
        let eye = state
            .drone
            .filter(|d| d.controlling)
            .map_or(state.position + Vec3::Y * state.tuning.eye_height, |d| {
                d.position
            });
        let enemy = contacts.get(bot).ok().and_then(|contact| contact.position);
        let distance = enemy.map_or(15.0, |p| p.distance(eye));
        let rating = |held: HeldWeapon| {
            let mut value = power(held, state.tuning, distance);
            if held.kind == WeaponKind::Drone {
                value /= 1.0
                    + state.weapon.cooldowns[WeaponKind::Drone.index()] as f32
                        * boltrunner_shared::TICK_DURATION.as_secs_f32();
            }
            value
        };
        let mut slot = state.weapon.active_slot;
        let mut best = state.weapon.held(slot).map_or(0.0, rating);
        // Finish an in-progress reload instead of cancelling it on every plan.
        for candidate in (0..=2).filter(|_| state.weapon.reload_ticks == 0) {
            let Some(held) = state.weapon.held(candidate) else {
                continue;
            };
            let value = rating(held);
            if value > best * 1.15 {
                slot = candidate;
                best = value;
            }
        }
        director.plans.insert(bot, WeaponPlan { slot });
        let mut candidates = Vec::new();
        for &(target, item) in &items {
            if item.position.distance_squared(state.position) > 35.0_f32.powi(2) {
                continue;
            }
            // Infinite reserves make duplicate guns irrelevant as loot.
            let utility = if pickup_slot(state, item.weapon).is_some() {
                // Knife and drone remain low priorities while dependable ranged
                // weapons win; a knife can still fill an empty secondary slot.
                match item.weapon.kind {
                    WeaponKind::Drone => 18.0,
                    WeaponKind::Karambit => 16.0,
                    _ => 45.0,
                }
            } else {
                continue;
            };
            candidates.push(BotLootTarget {
                collector: bot,
                target,
                position: item.position,
                utility,
            });
        }
        candidates.sort_by(|a, b| {
            let score = |offer: &BotLootTarget| {
                offer.utility / (1.0 + offer.position.distance(state.position) / 10.0)
            };
            score(b)
                .total_cmp(&score(a))
                .then(a.target.to_bits().cmp(&b.target.to_bits()))
        });
        offers.0.extend(candidates.into_iter().take(16));
    }
}
fn press_slot(input: &mut PlayerInput, state: &PlayerState, slot: u8) {
    if input.slot_press == state.weapon.last_slot_press {
        input.slot = slot;
        input.slot_press = input.slot_press.wrapping_add(1);
    }
}
fn control(
    director: Res<EquipmentDirector>,
    settings: Res<crate::tuning::ServerSettings>,
    weapons: Query<&GroundWeapon>,
    mut bots: Query<
        (
            Entity,
            &PlayerState,
            &BotLootIntent,
            &BotContact,
            &mut ActionState<PlayerInput>,
        ),
        With<BotPlayer>,
    >,
) {
    for (entity, state, intent, contact, mut input) in &mut bots {
        if state.death.is_some() || state.match_paused {
            input.0.alt = false;
            continue;
        }
        let Some(plan) = director.plans.get(&entity) else {
            continue;
        };
        if let Some(drone) = state.drone.filter(|d| d.controlling && !d.detonate) {
            input.0.fire = false;
            input.0.alt = false;
            input.0.left = false;
            input.0.right = false;
            input.0.backward = false;
            input.0.forward = true;
            if let Some(target) = contact.position {
                let aim = target - drone.position;
                // The brain already applies perception, reaction, error and turn
                // limits to the drone camera. Never replace its aim with a snap.
                if contact.ready
                    && aim.length() < 1.5
                    && input.0.fire_press == state.weapon.last_fire_press
                {
                    input.0.fire = true;
                    input.0.fire_press = input.0.fire_press.wrapping_add(1);
                }
            }
            continue;
        }
        if let Some(item) = intent.target.and_then(|target| weapons.get(target).ok()) {
            if reachable(state, item.position, settings.values.rules.pickup_radius) {
                if let Some(slot) = pickup_slot(state, item.weapon) {
                    press_slot(&mut input.0, state, slot);
                }
                continue;
            }
        }
        // inventory::collect restricts a bot's intentional swap to intent.target;
        // switching owned slots never accidentally consumes an unrelated drop.
        if plan.slot != state.weapon.active_slot {
            press_slot(&mut input.0, state, plan.slot);
        }
        if state.weapon.selected == WeaponKind::Drone
            && state.drone.is_none()
            && input.0.fire
            && state.weapon.switch_ticks == 0
            && state.weapon.cooldowns[WeaponKind::Drone.index()] == 0
            && input.0.fire_press == state.weapon.last_fire_press
        {
            input.0.fire_press = input.0.fire_press.wrapping_add(1);
        }
        if let Some(target) = contact.position {
            let distance = target.distance(state.position + Vec3::Y * state.tuning.eye_height);
            let stats = state.weapon.selected.stats(state.tuning);
            if matches!(
                state.weapon.selected,
                WeaponKind::Flame | WeaponKind::Karambit | WeaponKind::Arc
            ) && distance > stats.range
            {
                input.0.fire = false;
            }
            if stats.blast_radius > 0.0
                && distance < stats.blast_radius + state.tuning.player_radius + 1.0
            {
                input.0.fire = false;
            }
        }
    }
}
