use crate::{
    PlayerInput, PlayerState, TICK_DURATION, level,
    tuning::{SimulationTuning, seconds_to_ticks},
};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[repr(u8)]
pub enum WeaponKind {
    #[default]
    Pistol,
    Flame,
    Shotgun,
    MachineGun,
    Rocket,
    Laser,
    Sniper,
    Drone,
    Ricochet,
    Karambit,
    Arc,
    Grenade,
    ElectroPlasma,
    HomingPlasma,
}
impl WeaponKind {
    pub const ALL: [Self; 14] = [
        Self::Pistol,
        Self::Flame,
        Self::Shotgun,
        Self::MachineGun,
        Self::Rocket,
        Self::Laser,
        Self::Sniper,
        Self::Drone,
        Self::Ricochet,
        Self::Karambit,
        Self::Arc,
        Self::Grenade,
        Self::ElectroPlasma,
        Self::HomingPlasma,
    ];
    pub fn index(self) -> usize {
        self as usize
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Pistol => "Energy pistol",
            Self::Flame => "Flamethrower",
            Self::Shotgun => "Shotgun",
            Self::MachineGun => "Machine gun",
            Self::Rocket => "Rocket launcher",
            Self::Laser => "Laser cannon",
            Self::Sniper => "Sniper rifle",
            Self::Drone => "Drone controller",
            Self::Ricochet => "Ricochet cannon",
            Self::Karambit => "Pixel karambit",
            Self::Arc => "Arc cannon",
            Self::Grenade => "Plasma launcher",
            Self::ElectroPlasma => "Electro plasma rifle",
            Self::HomingPlasma => "Seeker plasma gun",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Pistol => "Accurate energy bolts",
            Self::Flame => "Flame jet / lingering fire",
            Self::Shotgun => "Random pellet spread / close range",
            Self::MachineGun => "Automatic / heat and spread",
            Self::Rocket => "Explosive rockets / rocket jumps",
            Self::Laser => "Charge / continuous tracking beam",
            Self::Sniper => "Precision shot / right-click scope",
            Self::Drone => "LMB: launch / detonate · RMB: exit",
            Self::Ricochet => "Bouncing balls / bank shots",
            Self::Karambit => "Fast melee / sweeping blade",
            Self::Arc => "Lightning / chains to nearby enemies",
            Self::Grenade => "Bouncing grenades / timed plasma blast",
            Self::ElectroPlasma => "Plasma orbs / lingering electrical fields",
            Self::HomingPlasma => "Slow plasma orbs / gentle target guidance",
        }
    }
    pub fn color(self) -> [f32; 3] {
        match self {
            Self::Pistol => [0.25, 0.8, 1.0],
            Self::Flame => [1.0, 0.35, 0.06],
            Self::Shotgun => [1.0, 0.75, 0.3],
            Self::MachineGun => [0.75, 0.9, 0.45],
            Self::Rocket => [1.0, 0.3, 0.2],
            Self::Laser => [0.5, 0.3, 1.0],
            Self::Sniper => [0.1, 0.95, 0.8],
            Self::Drone => [0.2, 0.6, 1.0],
            Self::Ricochet => [0.3, 1.0, 0.4],
            Self::Karambit => [0.35, 1.0, 0.9],
            Self::Arc => [0.3, 0.7, 1.0],
            Self::Grenade => [1.0, 0.25, 0.75],
            Self::ElectroPlasma => [0.10, 0.7, 1.0],
            Self::HomingPlasma => [0.5, 1.0, 0.3],
        }
    }
    pub fn is_projectile(self) -> bool {
        matches!(
            self,
            Self::Pistol
                | Self::Shotgun
                | Self::MachineGun
                | Self::Rocket
                | Self::Ricochet
                | Self::Grenade
                | Self::ElectroPlasma
                | Self::HomingPlasma
        )
    }
    pub fn is_stream(self) -> bool {
        matches!(self, Self::Flame | Self::Laser)
    }
    pub fn rebounds(self) -> bool {
        matches!(self, Self::Ricochet | Self::Grenade)
    }
    /// Streams store remaining simulation ticks; other guns store shots.
    pub fn ammo_limit(self, t: SimulationTuning) -> Option<u32> {
        Some(match self {
            Self::Karambit | Self::Drone => return None,
            Self::Pistol => t.pistol_ammo as u32,
            Self::Flame => (t.flame_fuel * crate::TICK_HZ as f32).round() as u32,
            Self::Laser => (t.laser_fuel * crate::TICK_HZ as f32).round() as u32,
            Self::Shotgun => t.shotgun_ammo as u32,
            Self::MachineGun => t.mg_ammo as u32,
            Self::Rocket => t.rocket_ammo as u32,
            Self::Sniper => t.sniper_ammo as u32,
            Self::Ricochet => t.ball_ammo as u32,
            Self::Arc => t.arc_ammo as u32,
            Self::Grenade => t.grenade_ammo as u32,
            Self::ElectroPlasma => t.electro_ammo as u32,
            Self::HomingPlasma => t.seeker_ammo as u32,
        })
    }

    pub fn loads_individually(self) -> bool {
        matches!(self, Self::Shotgun | Self::Rocket)
    }
    /// A full magazine exchange, or one shell/rocket insertion.
    pub fn reload_seconds(self, t: SimulationTuning) -> f32 {
        match self {
            Self::Pistol => t.pistol_reload,
            Self::Flame => t.flame_reload,
            Self::Shotgun => t.shotgun_reload,
            Self::MachineGun => t.mg_reload,
            Self::Rocket => t.rocket_reload,
            Self::Laser => t.laser_reload,
            Self::Sniper => t.sniper_reload,
            Self::Drone => 0.0,
            Self::Ricochet => t.ball_reload,
            Self::Karambit => 0.0,
            Self::Arc => t.arc_reload,
            Self::Grenade => t.grenade_reload,
            Self::ElectroPlasma => t.electro_reload,
            Self::HomingPlasma => t.seeker_reload,
        }
    }

    pub fn stats(self, t: SimulationTuning) -> GunStats {
        let mut s = GunStats {
            damage: t.projectile_damage as f32,
            interval: t.fire_interval,
            speed: t.projectile_speed,
            radius: t.projectile_radius,
            lifetime: t.projectile_lifetime,
            range: 100.0,
            pellets: 1,
            spread: 0.0,
            gravity: 0.0,
            bounces: 0,
            bounce_loss: 1.0,
            blast_radius: 0.0,
            blast_damage: 0.0,
            knockback: 0.0,
            self_damage: 1.0,
            falloff_start: 1e6,
            falloff_end: 1e6 + 1.0,
            min_damage: 1.0,
        };
        match self {
            Self::Pistol => {}
            Self::Flame => {
                s.damage = t.flame_dps * 0.05;
                s.interval = 0.05;
                s.range = t.flame_range;
                s.spread = t.flame_angle;
            }
            Self::Shotgun => {
                s.damage = t.shotgun_damage as f32;
                s.interval = t.shotgun_interval;
                s.speed = t.shotgun_speed;
                s.radius = t.shotgun_radius;
                s.lifetime = t.shotgun_lifetime;
                s.pellets = t.shotgun_pellets as u8;
                s.spread = t.shotgun_spread;
                s.falloff_start = t.shotgun_falloff_start;
                s.falloff_end = t.shotgun_falloff_end;
                s.min_damage = t.shotgun_min_damage;
            }
            Self::MachineGun => {
                s.damage = t.mg_damage as f32;
                s.interval = t.mg_interval;
                s.speed = t.mg_speed;
                s.radius = t.mg_radius;
                s.lifetime = t.mg_lifetime;
                s.spread = t.mg_spread;
            }
            Self::Rocket => {
                s.damage = t.rocket_damage as f32;
                s.interval = t.rocket_interval;
                s.speed = t.rocket_speed;
                s.radius = t.rocket_radius;
                s.lifetime = t.rocket_lifetime;
                s.blast_radius = t.rocket_blast_radius;
                s.blast_damage = t.rocket_blast_damage as f32;
                s.knockback = t.rocket_knockback;
                s.self_damage = t.rocket_self_damage;
            }
            Self::Laser => {
                s.damage = t.laser_dps * 0.05;
                s.interval = 0.05;
                s.radius = t.laser_radius;
                s.range = t.laser_range;
            }
            Self::Sniper => {
                s.damage = t.sniper_damage as f32;
                s.interval = t.sniper_interval;
                s.radius = t.sniper_radius;
                s.range = t.sniper_range;
            }
            Self::Drone => {
                s.damage = 0.0;
                s.interval = t.drone_cooldown;
            }
            Self::Ricochet => {
                s.damage = t.ball_damage as f32;
                s.interval = t.ball_interval;
                s.speed = t.ball_speed;
                s.radius = t.ball_radius;
                s.lifetime = t.ball_lifetime;
                s.gravity = t.ball_gravity;
                s.bounces = t.ball_bounces as u8;
                s.bounce_loss = t.ball_retention;
            }
            Self::Karambit => {
                s.damage = t.karambit_damage as f32;
                s.interval = t.karambit_interval;
                s.range = t.karambit_range;
                s.radius = 0.15;
            }
            Self::Arc => {
                s.damage = t.arc_damage as f32;
                s.interval = t.arc_interval;
                s.range = t.arc_range;
                s.radius = t.arc_radius;
            }
            Self::ElectroPlasma => {
                s.damage = t.electro_damage as f32;
                s.interval = t.electro_interval;
                s.speed = t.electro_speed;
                s.radius = t.electro_radius;
                s.lifetime = t.electro_lifetime;
            }
            Self::HomingPlasma => {
                s.damage = t.seeker_damage as f32;
                s.interval = t.seeker_interval;
                s.speed = t.seeker_speed;
                s.radius = t.seeker_radius;
                s.lifetime = t.seeker_lifetime;
            }
            Self::Grenade => {
                s.damage = t.grenade_damage as f32;
                s.interval = t.grenade_interval;
                s.speed = t.grenade_speed;
                s.radius = t.grenade_radius;
                s.lifetime = t.grenade_fuse;
                s.gravity = t.grenade_gravity;
                s.bounces = t.grenade_bounces as u8;
                s.bounce_loss = t.grenade_retention;
                s.blast_radius = t.grenade_blast_radius;
                s.blast_damage = t.grenade_blast_damage as f32;
                s.knockback = t.grenade_knockback;
                s.self_damage = t.grenade_self_damage;
            }
        }
        s
    }
}

#[derive(Clone, Copy)]
pub struct GunStats {
    pub damage: f32,
    pub interval: f32,
    pub speed: f32,
    pub radius: f32,
    pub lifetime: f32,
    pub range: f32,
    pub pellets: u8,
    pub spread: f32,
    pub gravity: f32,
    pub bounces: u8,
    pub bounce_loss: f32,
    pub blast_radius: f32,
    pub blast_damage: f32,
    pub knockback: f32,
    pub self_damage: f32,
    pub falloff_start: f32,
    pub falloff_end: f32,
    pub min_damage: f32,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Reflect)]
pub struct WeaponState {
    pub pistol_ammo: u32,
    pub reload_ticks: u16,
    pub reload_total_ticks: u16,
    pub last_reload_press: u16,
    pub slots: [Option<crate::inventory::HeldWeapon>; 2],
    pub active_slot: u8,
    pub last_slot_press: u16,
    pub last_dev_press: u16,
    pub last_shot_kind: WeaponKind,
    pub last_shot_charge: u16,
    pub selected: WeaponKind,
    pub cooldowns: [u16; WeaponKind::ALL.len()],
    pub switch_ticks: u16,
    pub charge_ticks: u16,
    pub heat: f32,
    pub overheated: bool,
    pub firing: bool,
    pub aim_pitch: f32,
    pub shot: u16,
    pub last_fire_press: u16,
}

impl Default for WeaponState {
    fn default() -> Self {
        Self::new(SimulationTuning::default())
    }
}
impl WeaponState {
    pub fn new(tuning: SimulationTuning) -> Self {
        Self {
            pistol_ammo: tuning.pistol_ammo as u32,
            reload_ticks: 0,
            reload_total_ticks: 0,
            last_reload_press: 0,
            slots: [None; 2],
            active_slot: 0,
            last_slot_press: 0,
            last_dev_press: 0,
            last_shot_kind: WeaponKind::Pistol,
            last_shot_charge: 0,
            selected: WeaponKind::Pistol,
            cooldowns: [0; WeaponKind::ALL.len()],
            switch_ticks: 0,
            charge_ticks: 0,
            heat: 0.0,
            overheated: false,
            firing: false,
            aim_pitch: 0.0,
            shot: 0,
            last_fire_press: 0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Reflect)]
pub struct DroneState {
    pub damage_fraction: f32,
    pub position: Vec3,
    pub previous_position: Vec3,
    pub velocity: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub health: u16,
    pub remaining_ticks: u16,
    pub owner_grace_ticks: u16,
    pub controlling: bool,
    pub detonate: bool,
}

pub fn flame_reach(state: &PlayerState) -> f32 {
    (state.weapon.charge_ticks as f32 * TICK_DURATION.as_secs_f32() * state.tuning.flame_speed)
        .min(state.tuning.flame_range)
}

pub fn step(state: &mut PlayerState, input: &PlayerInput) {
    crate::inventory::commands(state, input);
    crate::inventory::advance_reload(state, input);
    let kind = state.weapon.selected;
    let previous = state.weapon.shot;
    advance_weapon(state, input);
    let fired = state.weapon.shot != previous;
    if fired {
        state.weapon.last_shot_kind = kind;
        state.weapon.last_shot_charge = state.weapon.charge_ticks;
    }
    let consuming = if kind.is_stream() {
        state.weapon.firing
    } else {
        fired
    };
    if consuming {
        crate::inventory::consume(state);
    }
}

fn advance_weapon(state: &mut PlayerState, input: &PlayerInput) {
    let dt = TICK_DURATION.as_secs_f32();
    let t = state.tuning;
    let w = &mut state.weapon;
    for cooldown in &mut w.cooldowns {
        *cooldown = cooldown.saturating_sub(1);
    }
    w.switch_ticks = w.switch_ticks.saturating_sub(1);
    let pressed = input.fire_press != w.last_fire_press;
    w.last_fire_press = input.fire_press;
    w.aim_pitch = input.pitch_radians();
    if w.selected != WeaponKind::MachineGun || !input.fire || w.overheated || w.reload_ticks > 0 {
        w.heat = (w.heat - t.mg_cooling * dt).max(0.0);
    }
    if w.heat <= 0.05 {
        w.overheated = false;
    }
    w.firing = false;
    if let Some(drone) = &mut state.drone {
        drone.previous_position = drone.position;
        drone.remaining_ticks = drone.remaining_ticks.saturating_sub(1);
        drone.owner_grace_ticks = drone.owner_grace_ticks.saturating_sub(1);
        if pressed
            && (w.selected == WeaponKind::Drone || drone.controlling)
            && state.death.is_none()
        {
            if drone.controlling {
                drone.detonate = true;
                drone.controlling = false;
            } else if !input.alt {
                drone.controlling = true;
            }
        }
        if input.alt {
            drone.controlling = false;
        }
        if state.death.is_some() {
            drone.detonate = true;
            drone.controlling = false;
        }
        if drone.remaining_ticks == 0 {
            drone.detonate = true;
        }
        if drone.controlling && !drone.detonate {
            drone.yaw = input.yaw_radians() + state.view_yaw_offset;
            drone.pitch = input.pitch_radians();
            let rotation = Quat::from_euler(EulerRot::YXZ, drone.yaw, drone.pitch, 0.0);
            let axes = Vec3::new(
                input.right as u8 as f32 - input.left as u8 as f32,
                input.up as u8 as f32 - input.down as u8 as f32,
                input.backward as u8 as f32 - input.forward as u8 as f32,
            );
            let wish = rotation * Vec3::new(axes.x, 0.0, axes.z) + Vec3::Y * axes.y;
            drone.velocity = drone.velocity.move_towards(
                wish.normalize_or_zero() * t.drone_speed,
                t.drone_acceleration * dt,
            );
            let delta = drone.velocity * dt;
            if let Some(hit) = level::world().sweep_sphere(drone.position, delta, t.drone_radius) {
                drone.position += delta * hit;
                drone.detonate = true;
            } else {
                drone.position += delta;
            }
        } else {
            drone.velocity = Vec3::ZERO;
        }
        if w.selected == WeaponKind::Drone || drone.controlling || drone.detonate {
            return;
        }
    }
    if state.death.is_some()
        || w.switch_ticks > 0
        || w.reload_ticks > 0
        || !w.held(w.active_slot).is_some_and(|item| item.usable(t))
        || !input.fire
    {
        w.charge_ticks = 0;
        return;
    }
    if w.selected.is_stream() {
        w.charge_ticks = w.charge_ticks.saturating_add(1);
        if w.selected == WeaponKind::Laser && w.charge_ticks < seconds_to_ticks(t.laser_charge) {
            return;
        }
    }
    if w.selected == WeaponKind::MachineGun && w.overheated {
        return;
    }
    if w.selected == WeaponKind::Drone && !pressed {
        return;
    }
    w.firing = w.selected.is_stream();
    let slot = w.selected.index();
    if w.cooldowns[slot] > 0 {
        return;
    }
    w.shot = w.shot.wrapping_add(1);
    let rate = if state.rapid_fire_ticks > 0 {
        t.rapid_fire_multiplier
    } else {
        1.0
    };
    w.cooldowns[slot] = seconds_to_ticks(w.selected.stats(t).interval / rate).max(1);
    if w.selected == WeaponKind::MachineGun {
        w.heat = (w.heat + t.mg_heating * t.mg_interval).min(1.0);
        if w.heat >= 1.0 {
            w.overheated = true;
        }
    }
    if w.selected == WeaponKind::Drone {
        let rotation = Quat::from_euler(EulerRot::YXZ, state.yaw, input.pitch_radians(), 0.0);
        let origin = state.position + Vec3::Y * t.eye_height;
        let direction = rotation * Vec3::NEG_Z;
        // Release outside the owner's capsule, including downward aim.
        // Briefly ignore owner contact while their body settles after launch.
        let radius = t.player_radius + t.drone_radius + 0.04;
        let inside = |distance: f32| {
            let point = origin + direction * distance;
            let axis = Vec3::new(
                state.position.x,
                point.y.clamp(
                    state.position.y + t.player_radius,
                    state.position.y + t.player_height - t.player_radius,
                ),
                state.position.z,
            );
            point.distance_squared(axis) < radius * radius
        };
        let mut near = 0.0;
        let mut far = t.player_height + radius;
        for _ in 0..16 {
            let mid = (near + far) * 0.5;
            if inside(mid) {
                near = mid;
            } else {
                far = mid;
            }
        }
        let launch =
            level::world().clip_camera(origin, origin + direction * far.max(0.7), t.drone_radius);
        state.drone = Some(DroneState {
            damage_fraction: 0.0,
            previous_position: launch,
            position: launch,
            velocity: Vec3::ZERO,
            yaw: state.yaw,
            pitch: input.pitch_radians(),
            health: t.drone_health as u16,
            remaining_ticks: seconds_to_ticks(t.drone_lifetime).max(1),
            owner_grace_ticks: seconds_to_ticks(t.drone_owner_grace),
            controlling: true,
            detonate: false,
        });
    }
}
