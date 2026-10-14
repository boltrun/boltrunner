use crate::bots::{BotLootIntent, BotLootTarget, BotLootTargets, BotSystems};
use bevy::prelude::*;
use boltrunner_shared::{
    PlayerInput, PlayerState,
    arsenal::WeaponKind,
    inventory::{GroundWeapon, HeldWeapon, reachable},
    level,
    match_state::MatchState,
    protocol::BotPlayer,
    tuning::{FeatureSettings, SimulationTuning},
};
use lightyear::prelude::input::native::ActionState;
use std::collections::{HashMap, VecDeque};

pub struct BotEquipmentPlugin;
impl Plugin for BotEquipmentPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EquipmentDirector>()
            .add_observer(track_weapon)
            .add_systems(FixedUpdate, offer_weapons.in_set(BotSystems::Offers))
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
    drone_target: Option<Vec3>,
}

fn track_weapon(event: On<Add, GroundWeapon>, mut director: ResMut<EquipmentDirector>) {
    director.weapons.push_front(event.entity);
}

fn power(weapon: HeldWeapon, tuning: SimulationTuning, distance: f32) -> f32 {
    if weapon.kind == WeaponKind::Pistol {
        return 8.0;
    }
    let stats = weapon.kind.stats(tuning);
    let available = weapon.kind.ammo_limit(tuning).map_or(1.0, |limit| {
        (weapon.ammo as f32 / limit.max(1) as f32).clamp(0.0, 1.0)
    });
    let damage = (stats.damage * (stats.pellets as f32).sqrt()).max(stats.blast_damage * 0.65);
    let mut value =
        (20.0 + damage / stats.interval.max(0.06).sqrt() * 0.35) * (0.35 + available * 0.65);
    if matches!(
        weapon.kind,
        WeaponKind::Flame | WeaponKind::Karambit | WeaponKind::Arc
    ) && distance > stats.range * 0.85
    {
        value *= 0.1;
    }
    if stats.blast_radius > 0.0 && distance < stats.blast_radius + tuning.player_radius + 1.0 {
        value *= 0.08;
    }
    value
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
    round: Res<MatchState>,
    weapons: Query<(Entity, &GroundWeapon)>,
    bots: Query<(Entity, &PlayerState), With<BotPlayer>>,
    players: Query<(Entity, &PlayerState)>,
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
        let enemy = players
            .iter()
            .take(16)
            .filter(|(id, p)| *id != bot && p.death.is_none() && !p.match_paused)
            .filter_map(|(_, p)| {
                let center = p.position + Vec3::Y * p.tuning.player_height * 0.65;
                let offset = center - eye;
                (offset.length_squared() < 55.0_f32.powi(2)
                    && level::world().sweep_sphere(eye, offset, 0.02).is_none())
                .then_some(center)
            })
            .min_by(|a, b| a.distance_squared(eye).total_cmp(&b.distance_squared(eye)));
        let distance = enemy.map_or(15.0, |p| p.distance(eye));
        let mut slot = state.weapon.active_slot;
        let mut best = state
            .weapon
            .held(slot)
            .map_or(0.0, |w| power(w, state.tuning, distance));
        // Finish an in-progress reload instead of cancelling it on every plan.
        for candidate in (0..=2).filter(|_| state.weapon.reload_ticks == 0) {
            let Some(held) = state.weapon.held(candidate) else {
                continue;
            };
            let value = power(held, state.tuning, distance);
            if value > best * 1.15 {
                slot = candidate;
                best = value;
            }
        }
        director.plans.insert(
            bot,
            WeaponPlan {
                slot,
                drone_target: enemy,
            },
        );
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
    settings: Res<FeatureSettings>,
    weapons: Query<&GroundWeapon>,
    mut bots: Query<
        (
            Entity,
            &PlayerState,
            &BotLootIntent,
            &mut ActionState<PlayerInput>,
        ),
        With<BotPlayer>,
    >,
) {
    for (entity, state, intent, mut input) in &mut bots {
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
            if let Some(target) = plan.drone_target {
                let aim = target - drone.position;
                input.0.yaw =
                    PlayerInput::encode_yaw((-aim.x).atan2(-aim.z) - state.view_yaw_offset);
                input.0.pitch = PlayerInput::encode_pitch((aim.y / aim.length().max(0.001)).asin());
                if aim.length() < 1.5 && input.0.fire_press == state.weapon.last_fire_press {
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
        if let Some(target) = plan.drone_target {
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
