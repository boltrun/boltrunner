use crate::{
    PlayerInput, PlayerState,
    arsenal::{WeaponKind, WeaponState},
    tuning::{SimulationTuning, seconds_to_ticks},
};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Reflect)]
pub struct HeldWeapon {
    pub kind: WeaponKind,
    pub ammo: u32,
}
impl HeldWeapon {
    pub fn full(kind: WeaponKind, tuning: SimulationTuning) -> Self {
        Self {
            kind,
            ammo: kind.ammo_limit(tuning).unwrap_or(0),
        }
    }
    pub fn usable(self, tuning: SimulationTuning) -> bool {
        self.kind.ammo_limit(tuning).is_none() || self.ammo > 0
    }
    pub fn ammo_label(self, tuning: SimulationTuning) -> String {
        if self.kind.ammo_limit(tuning).is_none() {
            "∞".into()
        } else if self.kind.is_stream() {
            format!(
                "{:.1}/{:.1}s",
                self.ammo as f64 / crate::TICK_HZ,
                self.kind.ammo_limit(tuning).unwrap() as f64 / crate::TICK_HZ
            )
        } else {
            format!("{}/{}", self.ammo, self.kind.ammo_limit(tuning).unwrap())
        }
    }
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Reflect)]
pub struct GroundWeapon {
    pub weapon: HeldWeapon,
    pub position: Vec3,
    /// Only director-created weapons count toward the authored pickup limit.
    pub map_spawn: bool,
}

impl WeaponState {
    pub fn held(&self, slot: u8) -> Option<HeldWeapon> {
        if slot == 0 {
            Some(HeldWeapon {
                kind: WeaponKind::Pistol,
                ammo: self.pistol_ammo,
            })
        } else {
            self.slots.get(slot as usize - 1).copied().flatten()
        }
    }
    pub fn select_slot(&mut self, slot: u8, tuning: SimulationTuning) -> bool {
        let Some(weapon) = self.held(slot) else {
            return false;
        };
        if self.active_slot != slot || self.selected != weapon.kind {
            self.active_slot = slot;
            self.selected = weapon.kind;
            self.switch_ticks = seconds_to_ticks(tuning.weapon_switch_time);
            self.charge_ticks = 0;
            self.firing = false;
            self.reload_ticks = 0;
            self.reload_total_ticks = 0;
        }
        true
    }
}

pub fn commands(state: &mut PlayerState, input: &PlayerInput) {
    let w = &mut state.weapon;
    let developer = input.dev_weapon_press != w.last_dev_press;
    let change = input.slot_press != w.last_slot_press;
    let reload = input.reload_press != w.last_reload_press;
    w.last_reload_press = input.reload_press;
    w.last_dev_press = input.dev_weapon_press;
    w.last_slot_press = input.slot_press;
    if state.death.is_some() || state.match_paused {
        return;
    }
    let previous = w.active_slot;
    let previous_kind = w.selected;
    let manual = change && w.held(input.slot).is_some();
    if developer {
        w.reload_ticks = 0;
        w.reload_total_ticks = 0;
        if input.dev_weapon == WeaponKind::Pistol {
            w.select_slot(0, state.tuning);
        } else {
            let slot = w.active_slot.max(1).min(2);
            w.slots[slot as usize - 1] = Some(HeldWeapon::full(input.dev_weapon, state.tuning));
            // Re-select even when replacing an equipped gun of another kind.
            w.select_slot(slot, state.tuning);
        }
    }
    if change {
        w.select_slot(input.slot, state.tuning);
    }
    if (previous != w.active_slot || previous_kind != w.selected || manual)
        && let Some(drone) = &mut state.drone
    {
        drone.controlling = false;
    }
    if reload {
        start_reload(state);
    }
}

/// Reserves are infinite. Switching cancels only the unfinished insertion/exchange;
/// rounds already in the magazine remain attached to that weapon.
pub fn start_reload(state: &mut PlayerState) {
    let w = &mut state.weapon;
    let Some(capacity) = w.selected.ammo_limit(state.tuning) else {
        return;
    };
    if w.reload_ticks > 0
        || state.death.is_some()
        || state.match_paused
        || w.held(w.active_slot)
            .is_none_or(|item| item.ammo >= capacity)
    {
        return;
    }
    w.reload_total_ticks = seconds_to_ticks(w.selected.reload_seconds(state.tuning)).max(1);
    w.reload_ticks = w.reload_total_ticks;
    w.charge_ticks = 0;
    w.firing = false;
}

fn current_ammo_mut(w: &mut WeaponState) -> Option<&mut u32> {
    if w.active_slot == 0 {
        Some(&mut w.pistol_ammo)
    } else {
        w.slots
            .get_mut(w.active_slot as usize - 1)?
            .as_mut()
            .map(|item| &mut item.ammo)
    }
}

pub fn advance_reload(state: &mut PlayerState, input: &PlayerInput) {
    if state.death.is_some() || state.match_paused {
        return;
    }
    let w = &mut state.weapon;
    if w.reload_ticks > 0 {
        // An empty gun must load its first round before held fire can interrupt.
        if w.selected.loads_individually()
            && input.fire
            && w.held(w.active_slot).is_some_and(|item| item.ammo > 0)
        {
            w.reload_ticks = 0;
        } else {
            w.reload_ticks -= 1;
            if w.reload_ticks == 0 {
                let capacity = w.selected.ammo_limit(state.tuning).unwrap();
                let individual = w.selected.loads_individually();
                if let Some(ammo) = current_ammo_mut(w) {
                    *ammo = if individual {
                        (*ammo + 1).min(capacity)
                    } else {
                        capacity
                    };
                    if individual && *ammo < capacity && !input.fire {
                        w.reload_ticks = w.reload_total_ticks;
                    }
                }
            }
        }
    }
    if state
        .weapon
        .held(state.weapon.active_slot)
        .is_some_and(|item| !item.usable(state.tuning))
    {
        start_reload(state);
    }
}

pub fn consume(state: &mut PlayerState) {
    if state.weapon.selected.ammo_limit(state.tuning).is_none() {
        return;
    }
    if let Some(ammo) = current_ammo_mut(&mut state.weapon) {
        *ammo = ammo.saturating_sub(1);
    }
    if state
        .weapon
        .held(state.weapon.active_slot)
        .is_some_and(|item| item.ammo == 0)
    {
        let firing = state.weapon.firing;
        let charge = state.weapon.charge_ticks;
        start_reload(state);
        // The final stream tick still resolves with its original reach/charge.
        state.weapon.firing = firing;
        state.weapon.charge_ticks = charge;
    }
}

pub fn owns(w: &WeaponState, kind: WeaponKind) -> bool {
    kind == WeaponKind::Pistol || w.slots.iter().flatten().any(|item| item.kind == kind)
}

/// Pickup reach and the HUD prompt use the same wall-aware predicate.
pub fn reachable(state: &PlayerState, position: Vec3, radius: f32) -> bool {
    if state.death.is_some() || state.match_paused {
        return false;
    }
    let center = state.position + Vec3::Y * state.tuning.player_height * 0.5;
    center.distance_squared(position) <= radius * radius
        && crate::level::world()
            .sweep_sphere(center, position - center, 0.02)
            .is_none()
}
