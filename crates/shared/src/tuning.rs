use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Number {
    Int(i64),
    Float(f32),
}

impl std::fmt::Display for Number {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Int(value) => value.fmt(f),
            Self::Float(value) => value.fmt(f),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NumberKind {
    Int,
    Float,
}

impl NumberKind {
    pub fn parse(self, text: &str) -> Result<Number, String> {
        match self {
            Self::Int => text
                .trim()
                .parse::<i64>()
                .map(Number::Int)
                .map_err(|_| "Enter a whole number.".into()),
            Self::Float => text
                .trim()
                .parse::<f32>()
                .ok()
                .filter(|n| n.is_finite())
                .map(Number::Float)
                .ok_or_else(|| "Enter a finite number.".into()),
        }
    }
}

pub trait TunableNumber: Copy {
    const KIND: NumberKind;
    fn number(self) -> Number;
    fn from_number(value: Number) -> Result<Self, String>;
}
impl TunableNumber for f32 {
    const KIND: NumberKind = NumberKind::Float;
    fn number(self) -> Number {
        Number::Float(self)
    }
    fn from_number(value: Number) -> Result<Self, String> {
        match value {
            Number::Float(value) if value.is_finite() => Ok(value),
            _ => Err("Expected a finite float.".into()),
        }
    }
}
impl TunableNumber for i64 {
    const KIND: NumberKind = NumberKind::Int;
    fn number(self) -> Number {
        Number::Int(self)
    }
    fn from_number(value: Number) -> Result<Self, String> {
        match value {
            Number::Int(value) => Ok(value),
            _ => Err("Expected an integer.".into()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Category {
    Movement,
    Locomotion,
    Jumping,
    Dash,
    Collision,
    Player,
    Body,
    Health,
    Weapons,
    Pistol,
    Projectiles,
    Flamethrower,
    Shotgun,
    MachineGun,
    RocketLauncher,
    Laser,
    Sniper,
    Drone,
    Ricochet,
    Karambit,
    Arc,
    Grenade,
    ElectroPlasma,
    SeekerPlasma,
    World,
    Triggers,
    Spawning,
    Powerups,
    PickupEffects,
    PickupSpawning,
    Match,
    Camera,
    DeathCamera,
    Presentation,
    Viewmodel,
    Recoil,
    Effects,
    Display,
    Nameplates,
    Crosshair,
    KillFeed,
    Environment,
}
impl Category {
    pub const ALL: &[Self] = &[
        Self::Movement,
        Self::Locomotion,
        Self::Jumping,
        Self::Dash,
        Self::Collision,
        Self::Player,
        Self::Body,
        Self::Health,
        Self::Weapons,
        Self::Pistol,
        Self::Projectiles,
        Self::Flamethrower,
        Self::Shotgun,
        Self::MachineGun,
        Self::RocketLauncher,
        Self::Laser,
        Self::Sniper,
        Self::Drone,
        Self::Ricochet,
        Self::Karambit,
        Self::Arc,
        Self::Grenade,
        Self::ElectroPlasma,
        Self::SeekerPlasma,
        Self::World,
        Self::Triggers,
        Self::Spawning,
        Self::Powerups,
        Self::PickupEffects,
        Self::PickupSpawning,
        Self::Match,
        Self::Camera,
        Self::DeathCamera,
        Self::Presentation,
        Self::Viewmodel,
        Self::Recoil,
        Self::Effects,
        Self::Display,
        Self::Nameplates,
        Self::Crosshair,
        Self::KillFeed,
        Self::Environment,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Movement => "movement",
            Self::Locomotion => "locomotion",
            Self::Jumping => "jumping",
            Self::Dash => "dash",
            Self::Collision => "collision",
            Self::Player => "player",
            Self::Body => "body",
            Self::Health => "health",
            Self::Weapons => "weapons",
            Self::Pistol => "pistol",
            Self::Projectiles => "projectiles",
            Self::Flamethrower => "flamethrower",
            Self::Shotgun => "shotgun",
            Self::MachineGun => "machine_gun",
            Self::RocketLauncher => "rocket_launcher",
            Self::Laser => "laser",
            Self::Sniper => "sniper",
            Self::Drone => "drone",
            Self::Ricochet => "ricochet",
            Self::Karambit => "karambit",
            Self::Arc => "arc_cannon",
            Self::Grenade => "plasma_launcher",
            Self::ElectroPlasma => "electro_plasma",
            Self::SeekerPlasma => "seeker_plasma",
            Self::World => "world",
            Self::Triggers => "triggers",
            Self::Spawning => "spawning",
            Self::Powerups => "powerups",
            Self::PickupEffects => "effects",
            Self::PickupSpawning => "spawning",
            Self::Match => "match",
            Self::Camera => "camera",
            Self::DeathCamera => "death",
            Self::Presentation => "presentation",
            Self::Viewmodel => "viewmodel",
            Self::Recoil => "recoil",
            Self::Effects => "effects",
            Self::Display => "display",
            Self::Nameplates => "nameplates",
            Self::Crosshair => "crosshair",
            Self::KillFeed => "kill_feed",
            Self::Environment => "environment",
        }
    }
    pub fn parent(self) -> Option<Self> {
        match self {
            Self::Movement => None,
            Self::Locomotion => Some(Self::Movement),
            Self::Jumping => Some(Self::Movement),
            Self::Dash => Some(Self::Movement),
            Self::Collision => Some(Self::Movement),
            Self::Player => None,
            Self::Body => Some(Self::Player),
            Self::Health => Some(Self::Player),
            Self::Weapons => None,
            Self::Pistol => Some(Self::Weapons),
            Self::Projectiles => Some(Self::Weapons),
            Self::Flamethrower
            | Self::Shotgun
            | Self::MachineGun
            | Self::RocketLauncher
            | Self::Laser
            | Self::Sniper
            | Self::Drone
            | Self::Ricochet
            | Self::Karambit
            | Self::Arc
            | Self::Grenade
            | Self::ElectroPlasma
            | Self::SeekerPlasma => Some(Self::Weapons),
            Self::World => None,
            Self::Triggers => Some(Self::World),
            Self::Spawning => Some(Self::World),
            Self::Powerups => None,
            Self::PickupEffects => Some(Self::Powerups),
            Self::PickupSpawning => Some(Self::Powerups),
            Self::Match => None,
            Self::Camera => None,
            Self::DeathCamera => Some(Self::Camera),
            Self::Presentation => None,
            Self::Viewmodel => Some(Self::Presentation),
            Self::Recoil => Some(Self::Presentation),
            Self::Effects => Some(Self::Presentation),
            Self::Display => None,
            Self::Nameplates => Some(Self::Display),
            Self::Crosshair => Some(Self::Display),
            Self::KillFeed => Some(Self::Display),
            Self::Environment => None,
        }
    }
    pub fn is_within(self, ancestor: Self) -> bool {
        let mut current = Some(self);
        while let Some(category) = current {
            if category == ancestor {
                return true;
            }
            current = category.parent();
        }
        false
    }
    pub fn path(self) -> Vec<Self> {
        let mut path = vec![self];
        let mut current = self.parent();
        while let Some(category) = current {
            path.push(category);
            current = category.parent();
        }
        path.reverse();
        path
    }
}

pub struct ParameterSpec {
    pub id: Parameter,
    pub name: &'static str,
    pub category: Category,
    pub kind: NumberKind,
    pub default: Number,
    pub min: f64,
    pub max: f64,
    pub units: &'static str,
    pub help: &'static str,
}
impl ParameterSpec {
    pub fn validate(&self, number: Number) -> Result<(), String> {
        let value = match (self.kind, number) {
            (NumberKind::Int, Number::Int(value)) => value as f64,
            (NumberKind::Float, Number::Float(value)) if value.is_finite() => value as f64,
            _ => return Err("Invalid numeric type or non-finite value.".into()),
        };
        if value < self.min || value > self.max {
            return Err(format!(
                "{} must be between {} and {}.",
                self.name, self.min, self.max
            ));
        }
        Ok(())
    }
}

macro_rules! default_number {
    (f32, $value:expr) => {
        Number::Float($value)
    };
    (i64, $value:expr) => {
        Number::Int($value)
    };
}

macro_rules! tuning_parameters {
    ($($group:ident: $struct:ident { $($id:ident => $field:ident: $ty:ident = $default:expr, $category:ident, $min:expr, $max:expr, $units:expr, $help:expr;)+ })+) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
        #[repr(usize)]
        pub enum Parameter { $($($id,)+)+ }
        pub const PARAMETERS: &[ParameterSpec] = &[$($(ParameterSpec {
            id: Parameter::$id, name: stringify!($field), category: Category::$category,
            kind: <$ty as TunableNumber>::KIND, default: default_number!($ty, $default), min: $min, max: $max, units: $units, help: $help,
        },)+)+];
        $(#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Reflect)]
        pub struct $struct { $(pub $field: $ty,)+ }
        impl Default for $struct { fn default() -> Self { Self { $($field: $default,)+ } } })+
        #[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
        pub struct SettingsValues { $(pub $group: $struct,)+ }
        impl SettingsValues {
            pub fn get(&self, parameter: Parameter) -> Number {
                match parameter { $($(Parameter::$id => self.$group.$field.number(),)+)+ }
            }
            fn set(&mut self, parameter: Parameter, value: Number) -> Result<(), String> {
                parameter.spec().validate(value)?;
                let previous_height = self.simulation.player_height;
                let previous_eye = self.simulation.eye_height;
                match parameter { $($(Parameter::$id => self.$group.$field = <$ty as TunableNumber>::from_number(value)?,)+)+ }
                if parameter == Parameter::PlayerHeight {
                    self.simulation.eye_height = previous_eye * (self.simulation.player_height / previous_height);
                }
                Ok(())
            }
        }
    }
}

tuning_parameters! {
 simulation: SimulationTuning {

    WalkSpeed => walk_speed: f32 = 9.0, Locomotion, 0.0, 1000.0, "units/s", "Target horizontal movement speed. Dash and landing momentum can exceed this speed.";
    Acceleration => acceleration: f32 = 65.0, Locomotion, 0.0, 1000.0, "units/s²", "How quickly velocity approaches movement input, including direction changes and recovery from a dash. Applies on the ground and in the air.";
    Braking => braking: f32 = 80.0, Locomotion, 0.0, 1000.0, "units/s²", "How quickly horizontal velocity decreases when movement input is released. Applies on the ground and in the air.";
    JumpSpeed => jump_speed: f32 = 8.5, Jumping, 0.0, 1000.0, "units/s", "Initial upward velocity for ground and air jumps. Jump height also depends on gravity.";
    Gravity => gravity: f32 = 24.0, Jumping, 0.0, 1000.0, "units/s²", "Downward acceleration outside a dash. Lower values create longer, higher jumps.";
    DashSpeed => dash_speed: f32 = 24.0, Dash, 0.0, 1000.0, "units/s", "Total dash speed, including vertical movement. Nominal distance is dash_speed × dash_time before collisions.";
    DashTime => dash_time: f32 = 0.15, Dash, 0.0, 60.0, "seconds", "Dash duration, rounded to simulation ticks. Zero disables dashing. An active dash keeps the duration chosen when it started.";
    DashCharges => dash_charges: i64 = 2, Dash, 0.0, 255.0, "charges", "Maximum stored dash charges. Each press spends one charge and can redirect an active dash. Charges recharge sequentially, including in the air. Zero disables new dashes. Lowering the limit clamps stored charges; extra slots recharge when the limit increases.";
    DashRechargeTime => dash_recharge_time: f32 = 0.75, Dash, 0.0, 60.0, "seconds", "Time to restore one charge, rounded to simulation ticks. Recharge begins when the first charge is spent and continues while airborne or dashing. Further dashes do not restart it. Zero removes the recharge wait. Changes apply to the next recharge interval.";
    AirJumps => air_jumps: i64 = 1, Jumping, 0.0, 255.0, "jumps", "Extra jumps available between landings. Jump pads also restore these. Lowering the limit clamps remaining jumps; increases are filled on the next landing.";
    DashUpwardRatio => dash_upward_ratio: f32 = 1.0 / 3.0, Dash, 0.0, 10.0, "ratio", "Maximum upward velocity divided by horizontal velocity during a dash. Zero makes upward dashes horizontal; downward aim is unrestricted.";
    DashLandingMultiplier => dash_landing_multiplier: f32 = 1.0, Dash, 0.0, 10.0, "multiplier", "Forward speed after a downward dash hits the ground, multiplied by incoming total speed.";
    StepHeight => step_height: f32 = 0.45, Collision, 0.0, 3.0, "units", "Maximum stair height climbed automatically while grounded. Also controls downward floor following while moving.";
    WalkableNormal => walkable_normal_y: f32 = 0.65, Collision, 0.01, 1.0, "ratio", "Minimum upward component of a floor normal. Lower values allow steeper slopes; 1 only allows flat floors. The default is about 49.5 degrees from horizontal.";
    GroundProbe => ground_probe_distance: f32 = 0.025, Collision, 0.001, 1.0, "units", "Downward support check used to allow ground jumps and replenish air jumps.";
    GroundSnap => ground_snap_distance: f32 = 0.03, Collision, 0.001, 1.0, "units", "Additional downward floor-following distance. Moving players also add step_height.";
    PlayerRadius => player_radius: f32 = 0.4, Body, 0.05, 3.0, "units", "Capsule radius for world collision, projectile hits and map triggers. Height must be at least twice this value. Changing size inside tight geometry may require respawning.";
    PlayerHeight => player_height: f32 = 1.8, Body, 0.1, 10.0, "units", "Full capsule height from the feet. Changing it proportionally scales eye_height, including the camera and shot origin. Must be at least twice player_radius.";
    EyeHeight => eye_height: f32 = 1.6, Body, 0.0, 10.0, "units", "Camera and shot origin height above the feet. Automatically scales when player_height changes; can also be adjusted independently. Must not exceed player_height.";
    MaxHealth => max_health: i64 = 100, Health, 1.0, 65535.0, "HP", "Full health on spawn. Lowering this clamps current health; raising it does not heal existing players.";
    RespawnTime => respawn_time: f32 = 3.0, Health, 0.0, 60.0, "seconds", "Delay after death, rounded to simulation ticks. Zero respawns on the next tick. Existing deaths keep their original delay.";
    FireInterval => fire_interval: f32 = 0.2, Pistol, 0.0, 60.0, "seconds", "Minimum interval between automatic shots, rounded to simulation ticks. Zero fires once per simulation tick. Existing cooldowns keep their duration.";
    ProjectileDamage => projectile_damage: i64 = 25, Projectiles, 0.0, 65535.0, "HP", "Damage per confirmed player hit. New shots capture this value; existing projectiles keep their damage.";
    ProjectileSpeed => projectile_speed: f32 = 120.0, Projectiles, 0.0, 1000.0, "units/s", "Projectile travel speed. New shots capture this value.";
    ProjectileRadius => projectile_radius: f32 = 0.045, Projectiles, 0.001, 3.0, "units", "Collision sphere radius for both walls and players. New shots capture this value; visual beam width is independent.";
    ProjectileLifetime => projectile_lifetime: f32 = 2.0, Projectiles, 0.001, 60.0, "seconds", "Maximum projectile lifetime, rounded to simulation ticks. New shots capture this value. Range is speed multiplied by lifetime.";

    PistolAmmo => pistol_ammo: i64 = 12, Pistol, 1.0, 65535.0, "shots", "Pistol magazine capacity; reserves are infinite.";
    PistolReload => pistol_reload: f32 = 0.9, Pistol, 0.01, 30.0, "seconds", "Time for a full magazine exchange or recharge. Switching cancels the unfinished reload and preserves current ammunition.";
    FlameReload => flame_reload: f32 = 1.6, Flamethrower, 0.01, 30.0, "seconds", "Time for a full magazine exchange or recharge. Switching cancels the unfinished reload and preserves current ammunition.";
    ShotgunReload => shotgun_reload: f32 = 0.55, Shotgun, 0.01, 30.0, "seconds", "Time to insert one round; firing can interrupt after an insertion.";
    MgReload => mg_reload: f32 = 1.8, MachineGun, 0.01, 30.0, "seconds", "Time for a full magazine exchange or recharge. Switching cancels the unfinished reload and preserves current ammunition.";
    RocketReload => rocket_reload: f32 = 1.1, RocketLauncher, 0.01, 30.0, "seconds", "Time to insert one round; firing can interrupt after an insertion.";
    LaserReload => laser_reload: f32 = 1.4, Laser, 0.01, 30.0, "seconds", "Time for a full magazine exchange or recharge. Switching cancels the unfinished reload and preserves current ammunition.";
    SniperReload => sniper_reload: f32 = 1.5, Sniper, 0.01, 30.0, "seconds", "Time for a full magazine exchange or recharge. Switching cancels the unfinished reload and preserves current ammunition.";
    BallReload => ball_reload: f32 = 1.1, Ricochet, 0.01, 30.0, "seconds", "Time for a full magazine exchange or recharge. Switching cancels the unfinished reload and preserves current ammunition.";
    ArcReload => arc_reload: f32 = 1.0, Arc, 0.01, 30.0, "seconds", "Time for a full magazine exchange or recharge. Switching cancels the unfinished reload and preserves current ammunition.";
    GrenadeReload => grenade_reload: f32 = 1.4, Grenade, 0.01, 30.0, "seconds", "Time for a full magazine exchange or recharge. Switching cancels the unfinished reload and preserves current ammunition.";
    ElectroReload => electro_reload: f32 = 1.3, ElectroPlasma, 0.01, 30.0, "seconds", "Time for a full magazine exchange or recharge. Switching cancels the unfinished reload and preserves current ammunition.";
    SeekerReload => seeker_reload: f32 = 1.25, SeekerPlasma, 0.01, 30.0, "seconds", "Time for a full magazine exchange or recharge. Switching cancels the unfinished reload and preserves current ammunition.";
    FlameFuel => flame_fuel: f32 = 10.0, Flamethrower, 0.1, 500.0, "seconds", "Magazine fuel capacity in active flame seconds; reserves are infinite.";
    LaserFuel => laser_fuel: f32 = 8.0, Laser, 0.1, 500.0, "seconds", "Magazine charge capacity in active beam seconds; charging and idle time consume none. Reserves are infinite.";
    ShotgunAmmo => shotgun_ammo: i64 = 8, Shotgun, 1.0, 65535.0, "shots", "Magazine capacity. New pickups and reloads fill to this limit; reserves are infinite. Existing magazines retain their rounds.";
    MgAmmo => mg_ammo: i64 = 60, MachineGun, 1.0, 65535.0, "shots", "Magazine capacity. New pickups and reloads fill to this limit; reserves are infinite. Existing magazines retain their rounds.";
    RocketAmmo => rocket_ammo: i64 = 3, RocketLauncher, 1.0, 65535.0, "shots", "Magazine capacity. New pickups and reloads fill to this limit; reserves are infinite. Existing magazines retain their rounds.";
    SniperAmmo => sniper_ammo: i64 = 10, Sniper, 1.0, 65535.0, "shots", "Magazine capacity. New pickups and reloads fill to this limit; reserves are infinite. Existing magazines retain their rounds.";
    BallAmmo => ball_ammo: i64 = 20, Ricochet, 1.0, 65535.0, "shots", "Magazine capacity. New pickups and reloads fill to this limit; reserves are infinite. Existing magazines retain their rounds.";
    ArcAmmo => arc_ammo: i64 = 16, Arc, 1.0, 65535.0, "shots", "Magazine capacity. New pickups and reloads fill to this limit; reserves are infinite. Existing magazines retain their rounds.";
    GrenadeAmmo => grenade_ammo: i64 = 12, Grenade, 1.0, 65535.0, "shots", "Magazine capacity. New pickups and reloads fill to this limit; reserves are infinite. Existing magazines retain their rounds.";
    ElectroAmmo => electro_ammo: i64 = 60, ElectroPlasma, 1.0, 65535.0, "shots", "Magazine capacity. New pickups and reloads fill to this limit; reserves are infinite. Existing magazines retain their rounds.";
    SeekerAmmo => seeker_ammo: i64 = 40, SeekerPlasma, 1.0, 65535.0, "shots", "Magazine capacity. New pickups and reloads fill to this limit; reserves are infinite. Existing magazines retain their rounds.";
    ElectroDamage => electro_damage: i64 = 18, ElectroPlasma, 0.0, 65535.0, "HP", "Electro plasma direct-hit damage.";
    ElectroInterval => electro_interval: f32 = 0.18, ElectroPlasma, 0.01, 30.0, "seconds", "Interval between electro plasma shots.";
    ElectroSpeed => electro_speed: f32 = 45.0, ElectroPlasma, 0.1, 1000.0, "units/s", "Electro plasma orb speed.";
    ElectroRadius => electro_radius: f32 = 0.12, ElectroPlasma, 0.01, 3.0, "units", "Electro plasma orb collision radius.";
    ElectroLifetime => electro_lifetime: f32 = 4.0, ElectroPlasma, 0.1, 500.0, "seconds", "Maximum orb flight time.";
    ElectroFieldTime => electro_field_time: f32 = 2.0, ElectroPlasma, 0.0, 500.0, "seconds", "Electrical field lifetime after a wall impact.";
    ElectroFieldRadius => electro_field_radius: f32 = 1.5, ElectroPlasma, 0.1, 10.0, "units", "Electrical field damage radius; walls block damage.";
    ElectroFieldDps => electro_field_dps: f32 = 14.0, ElectroPlasma, 0.0, 1000.0, "HP/s", "Field damage per second, including damage to its owner.";
    SeekerDamage => seeker_damage: i64 = 28, SeekerPlasma, 0.0, 65535.0, "HP", "Seeker plasma direct-hit damage.";
    SeekerInterval => seeker_interval: f32 = 0.25, SeekerPlasma, 0.01, 30.0, "seconds", "Interval between seeker plasma shots.";
    SeekerSpeed => seeker_speed: f32 = 18.0, SeekerPlasma, 0.1, 1000.0, "units/s", "Slow orb speed; movement and dashes can dodge it.";
    SeekerRadius => seeker_radius: f32 = 0.13, SeekerPlasma, 0.01, 3.0, "units", "Seeker plasma orb collision radius.";
    SeekerLifetime => seeker_lifetime: f32 = 5.0, SeekerPlasma, 0.1, 500.0, "seconds", "Maximum seeker orb flight time.";
    SeekerTurn => seeker_turn: f32 = 25.0, SeekerPlasma, 0.0, 360.0, "degrees/s", "Maximum steering speed, without target leading or obstacle avoidance.";
    SeekerRange => seeker_range: f32 = 40.0, SeekerPlasma, 0.1, 1000.0, "units", "Maximum distance to a visible steering target.";
    SeekerCone => seeker_cone: f32 = 35.0, SeekerPlasma, 0.0, 89.0, "degrees", "Half-angle of the forward target acquisition cone.";
    KarambitAngle => karambit_angle: f32 = 120.0, Karambit, 1.0, 180.0, "degrees", "Full width of the melee sector; every visible target in reach can be hit once.";
    WeaponSwitchTime => weapon_switch_time: f32 = 0.25, Weapons, 0.0, 5.0, "seconds", "Delay before firing a newly selected weapon. Cooldowns and machine-gun heat survive switching.";
    FlameDps => flame_dps: f32 = 80.0 / 3.0, Flamethrower, 0.0, 1000.0, "HP/s", "Damage per second inside the flame cone. Fractional damage is accumulated on the server.";
    FlameRange => flame_range: f32 = 6.0, Flamethrower, 0.1, 100.0, "units", "Maximum reach of the flame cone; walls block damage.";
    FlameSpeed => flame_speed: f32 = 18.0, Flamethrower, 0.1, 200.0, "units/s", "Speed at which the flame jet reaches full length after firing starts.";
    FlameAngle => flame_angle: f32 = 18.0, Flamethrower, 0.1, 80.0, "degrees", "Half-angle of the flame cone.";
    FlameBurnDps => flame_burn_dps: f32 = 10.0 / 3.0, Flamethrower, 0.0, 1000.0, "HP/s", "Lingering damage after the victim leaves the flame cone.";
    FlameBurnTime => flame_burn_time: f32 = 1.0, Flamethrower, 0.0, 30.0, "seconds", "Burn duration; another contact refreshes it.";
    ShotgunDamage => shotgun_damage: i64 = 12, Shotgun, 0.0, 65535.0, "HP/pellet", "Damage of each pellet before distance falloff.";
    ShotgunPellets => shotgun_pellets: i64 = 10, Shotgun, 1.0, 32.0, "pellets", "Independent random pellet directions per shot.";
    ShotgunSpread => shotgun_spread: f32 = 9.0, Shotgun, 0.0, 60.0, "degrees", "Half-angle of a uniformly sampled spread cone.";
    ShotgunInterval => shotgun_interval: f32 = 0.9, Shotgun, 0.01, 10.0, "seconds", "Interval between shotgun blasts.";
    ShotgunSpeed => shotgun_speed: f32 = 120.0, Shotgun, 0.1, 1000.0, "units/s", "Speed of new pellets.";
    ShotgunRadius => shotgun_radius: f32 = 0.025, Shotgun, 0.001, 1.0, "units", "Pellet collision radius.";
    ShotgunLifetime => shotgun_lifetime: f32 = 0.4, Shotgun, 0.01, 10.0, "seconds", "Pellet flight lifetime.";
    ShotgunFalloffStart => shotgun_falloff_start: f32 = 8.0, Shotgun, 0.0, 500.0, "units", "Distance at which pellet damage begins to fall.";
    ShotgunFalloffEnd => shotgun_falloff_end: f32 = 20.0, Shotgun, 0.01, 1000.0, "units", "Distance at which pellet damage reaches the minimum fraction.";
    ShotgunMinDamage => shotgun_min_damage: f32 = 0.3, Shotgun, 0.0, 1.0, "fraction", "Minimum fraction of pellet damage at long range.";
    MgDamage => mg_damage: i64 = 14, MachineGun, 0.0, 65535.0, "HP", "Damage per machine-gun bullet.";
    MgInterval => mg_interval: f32 = 1.0 / 12.0, MachineGun, 0.01, 10.0, "seconds", "Time between automatic bullets.";
    MgSpeed => mg_speed: f32 = 160.0, MachineGun, 0.1, 1000.0, "units/s", "Speed of new bullets.";
    MgRadius => mg_radius: f32 = 0.025, MachineGun, 0.001, 1.0, "units", "Machine-gun bullet collision radius.";
    MgLifetime => mg_lifetime: f32 = 2.0, MachineGun, 0.01, 30.0, "seconds", "Bullet flight lifetime.";
    MgSpread => mg_spread: f32 = 0.5, MachineGun, 0.0, 60.0, "degrees", "Cold-barrel spread half-angle.";
    MgMaxSpread => mg_max_spread: f32 = 4.0, MachineGun, 0.0, 60.0, "degrees", "Spread half-angle at full heat.";
    MgHeating => mg_heating: f32 = 0.4, MachineGun, 0.0, 10.0, "heat/s", "Heat gain per second of firing. Full heat blocks firing.";
    MgCooling => mg_cooling: f32 = 0.65, MachineGun, 0.01, 10.0, "heat/s", "Cooling while released, holstered or overheated.";
    RocketDamage => rocket_damage: i64 = 0, RocketLauncher, 0.0, 65535.0, "HP", "Direct-impact damage, added to splash damage.";
    RocketInterval => rocket_interval: f32 = 0.55, RocketLauncher, 0.01, 30.0, "seconds", "Launch cooldown starts when a rocket is fired. Flight time and explosion do not restart it.";
    RocketSpeed => rocket_speed: f32 = 28.0, RocketLauncher, 0.1, 1000.0, "units/s", "Rocket flight speed.";
    RocketRadius => rocket_radius: f32 = 0.16, RocketLauncher, 0.01, 2.0, "units", "Rocket collision radius.";
    RocketLifetime => rocket_lifetime: f32 = 5.0, RocketLauncher, 0.01, 60.0, "seconds", "Rocket flight lifetime; expiry also explodes.";
    RocketBlastRadius => rocket_blast_radius: f32 = 4.0, RocketLauncher, 0.1, 50.0, "units", "Splash reach, blocked by level geometry.";
    RocketBlastDamage => rocket_blast_damage: i64 = 120, RocketLauncher, 0.0, 65535.0, "HP", "Maximum splash damage with linear falloff to the edge.";
    RocketKnockback => rocket_knockback: f32 = 14.0, RocketLauncher, 0.0, 100.0, "units/s", "Maximum blast impulse; supports rocket jumps.";
    RocketSelfDamage => rocket_self_damage: f32 = 1.0, RocketLauncher, 0.0, 2.0, "fraction", "Fraction of splash damage applied to the shooter.";
    LaserDps => laser_dps: f32 = 75.0, Laser, 0.0, 1000.0, "HP/s", "Continuous beam damage; stops at the first target or wall.";
    LaserRange => laser_range: f32 = 40.0, Laser, 0.1, 1000.0, "units", "Maximum beam length.";
    LaserRadius => laser_radius: f32 = 0.06, Laser, 0.001, 1.0, "units", "Beam hit radius; the visible beam uses the same diameter.";
    LaserCharge => laser_charge: f32 = 0.35, Laser, 0.0, 10.0, "seconds", "Hold-to-fire charge time; releasing or switching cancels it.";
    SniperDamage => sniper_damage: i64 = 100, Sniper, 0.0, 65535.0, "HP", "Damage of a precise instant shot; no separate headshot hitbox.";
    SniperInterval => sniper_interval: f32 = 1.4, Sniper, 0.01, 30.0, "seconds", "Interval between sniper shots.";
    SniperRange => sniper_range: f32 = 200.0, Sniper, 0.1, 2000.0, "units", "Maximum instant-shot distance.";
    SniperRadius => sniper_radius: f32 = 0.015, Sniper, 0.001, 1.0, "units", "Radius of the sniper line hit test.";
    SniperScope => sniper_scope_zoom: f32 = 4.5, Sniper, 1.0, 30.0, "x", "Magnification relative to the current world FOV while holding right mouse. 1x disables zoom; larger values always narrow the view.";
    DroneHealth => drone_health: i64 = 30, Drone, 1.0, 65535.0, "HP", "Health of a newly launched drone; destroying it detonates it.";
    DroneSpeed => drone_speed: f32 = 14.0, Drone, 0.1, 200.0, "units/s", "Pilot movement speed.";
    DroneAcceleration => drone_acceleration: f32 = 50.0, Drone, 0.1, 1000.0, "units/s²", "Drone flight acceleration.";
    DroneRadius => drone_radius: f32 = 0.25, Drone, 0.05, 2.0, "units", "Drone collision and shooting-target radius.";
    DroneLifetime => drone_lifetime: f32 = 120.0, Drone, 0.1, 500.0, "seconds", "Flight lifetime before automatic detonation.";
    DroneOwnerGrace => drone_owner_grace: f32 = 2.0, Drone, 0.0, 10.0, "seconds", "Owner protection after launch: ignore owner contact and the drone's blast damage and impulse. Other targets and collisions remain active.";
    DroneDamage => drone_damage: i64 = 150, Drone, 0.0, 65535.0, "HP", "Explosion damage in the inner half-radius: lethal at default HP. Outer half falls to zero.";
    DroneBlastRadius => drone_blast_radius: f32 = 3.0, Drone, 0.1, 50.0, "units", "Drone blast radius; walls block damage.";
    DroneKnockback => drone_knockback: f32 = 12.0, Drone, 0.0, 100.0, "units/s", "Drone blast impulse.";
    DroneSelfDamage => drone_self_damage: f32 = 0.5, Drone, 0.0, 2.0, "fraction", "Drone blast damage applied to its owner.";
    DroneCooldown => drone_cooldown: f32 = 0.0, Drone, 0.0, 500.0, "seconds", "Cooldown after a drone is destroyed or detonated.";
    BallDamage => ball_damage: i64 = 35, Ricochet, 0.0, 65535.0, "HP", "Damage per bouncing ball hit. After a bounce the owner can also be hit.";
    BallInterval => ball_interval: f32 = 0.35, Ricochet, 0.01, 30.0, "seconds", "Interval between balls.";
    BallSpeed => ball_speed: f32 = 22.0, Ricochet, 0.1, 1000.0, "units/s", "Initial ball speed.";
    BallRadius => ball_radius: f32 = 0.18, Ricochet, 0.01, 2.0, "units", "Ball visual and collision radius.";
    BallLifetime => ball_lifetime: f32 = 4.0, Ricochet, 0.01, 60.0, "seconds", "Maximum ball lifetime.";
    BallGravity => ball_gravity: f32 = 5.0, Ricochet, 0.0, 100.0, "units/s²", "Ball downward acceleration.";
    BallBounces => ball_bounces: i64 = 3, Ricochet, 0.0, 20.0, "bounces", "Allowed wall reflections before destruction.";
    BallRetention => ball_retention: f32 = 0.9, Ricochet, 0.1, 1.0, "fraction", "Speed retained after each bounce.";
    KarambitDamage => karambit_damage: i64 = 60, Karambit, 0.0, 65535.0, "HP", "Damage per blade strike; walls block the sweep.";
    KarambitInterval => karambit_interval: f32 = 0.4, Karambit, 0.05, 30.0, "seconds", "Interval between melee strikes.";
    KarambitRange => karambit_range: f32 = 2.5, Karambit, 0.1, 10.0, "units", "Blade reach from the player's eye.";
    ArcDamage => arc_damage: i64 = 36, Arc, 0.0, 65535.0, "HP", "Damage to the first lightning target.";
    ArcInterval => arc_interval: f32 = 0.55, Arc, 0.01, 30.0, "seconds", "Interval between lightning discharges.";
    ArcRange => arc_range: f32 = 28.0, Arc, 0.1, 1000.0, "units", "Maximum range of the aimed discharge.";
    ArcRadius => arc_radius: f32 = 0.055, Arc, 0.001, 1.0, "units", "First lightning segment hit radius.";
    ArcChainRange => arc_chain_range: f32 = 6.0, Arc, 0.0, 50.0, "units", "Maximum distance to each subsequent target; walls block chaining.";
    ArcChains => arc_chains: i64 = 2, Arc, 0.0, 8.0, "targets", "Additional targets after the first hit. Each target can be hit once per discharge.";
    ArcRetention => arc_retention: f32 = 0.7, Arc, 0.0, 1.0, "fraction", "Damage retained for each additional lightning target.";
    GrenadeDamage => grenade_damage: i64 = 20, Grenade, 0.0, 65535.0, "HP", "Direct damage on contact with a player or drone, before blast damage.";
    GrenadeInterval => grenade_interval: f32 = 0.8, Grenade, 0.01, 30.0, "seconds", "Launch interval; starts at launch rather than detonation.";
    GrenadeSpeed => grenade_speed: f32 = 24.0, Grenade, 0.1, 1000.0, "units/s", "Initial grenade speed.";
    GrenadeRadius => grenade_radius: f32 = 0.13, Grenade, 0.01, 2.0, "units", "Visible sphere and collision radius.";
    GrenadeGravity => grenade_gravity: f32 = 12.0, Grenade, 0.0, 100.0, "units/s²", "Downward acceleration of the grenade.";
    GrenadeFuse => grenade_fuse: f32 = 2.2, Grenade, 0.01, 30.0, "seconds", "Fuse starts at launch; wall bounces do not reset it.";
    GrenadeBounces => grenade_bounces: i64 = 12, Grenade, 0.0, 30.0, "bounces", "Maximum wall reflections before detonation.";
    GrenadeRetention => grenade_retention: f32 = 0.65, Grenade, 0.1, 1.0, "fraction", "Speed retained after a wall bounce.";
    GrenadeBlastRadius => grenade_blast_radius: f32 = 3.5, Grenade, 0.1, 50.0, "units", "Plasma blast radius; damage decreases with distance and walls block it.";
    GrenadeBlastDamage => grenade_blast_damage: i64 = 100, Grenade, 0.0, 65535.0, "HP", "Maximum blast damage at the grenade center.";
    GrenadeKnockback => grenade_knockback: f32 = 16.0, Grenade, 0.0, 100.0, "units/s", "Plasma blast impulse.";
    GrenadeSelfDamage => grenade_self_damage: f32 = 0.5, Grenade, 0.0, 2.0, "fraction", "Blast damage multiplier for the grenade's owner.";
    TriggerCooldown => trigger_cooldown: f32 = 0.3, Triggers, 0.0, 60.0, "seconds", "Shared delay between jump pads and portals. Hazards always remain active. Existing waits keep their original duration.";
    JumpPadMultiplier => jump_pad_multiplier: f32 = 1.0, Triggers, 0.0, 10.0, "multiplier", "Multiplies the map-authored launch velocity of every jump pad.";
    PortalMomentum => portal_momentum_multiplier: f32 = 1.0, Triggers, 0.0, 10.0, "multiplier", "Multiplies preserved horizontal and vertical velocity through teleports and warp portals. An active dash resumes its configured speed on the next tick.";
    KillPlaneHeight => kill_plane_height: f32 = -40.0, World, -1000.0, 1000.0, "units", "Players below this world-space height die immediately. Map-authored hurt volumes remain active.";
    SpawnSeparation => spawn_separation: f32 = 2.0, Spawning, 0.0, 100.0, "units", "Preferred distance from existing players when choosing a joining player spawn. If no spawn qualifies, the farthest available spawn is used. Respawns cycle authored locations.";
    HealthPickupFraction => health_pickup_fraction: f32 = 1.0, PickupEffects, 0.0, 1.0, "fraction", "Health restored on pickup as a fraction of maximum health. Full-health players still consume the pickup.";
    SpeedDuration => speed_duration: f32 = 8.0, PickupEffects, 0.0, 500.0, "seconds", "Duration of a speed pickup, rounded to simulation ticks. Picking up another refreshes the timer.";
    SpeedMultiplier => speed_multiplier: f32 = 1.65, PickupEffects, 0.0, 10.0, "multiplier", "Multiplier for walking speed and acceleration while speed boost is active.";
    RapidFireDuration => rapid_fire_duration: f32 = 8.0, PickupEffects, 0.0, 500.0, "seconds", "Duration of a rapid-fire pickup, rounded to simulation ticks. Picking up another refreshes the timer.";
    RapidFireMultiplier => rapid_fire_multiplier: f32 = 2.0, PickupEffects, 0.1, 10.0, "multiplier", "Fire rate multiplier while rapid fire is active. New cooldowns use the current value.";
 }
 rules: MatchTuning {
    MatchDuration => match_duration: f32 = 300.0, Match, 0.1, 86400.0, "seconds", "Active match duration. Live changes preserve elapsed time and can end the current match immediately.";
    ResultsDuration => results_duration: f32 = 10.0, Match, 0.1, 3600.0, "seconds", "Results duration before the next match. Live changes preserve elapsed time.";
    KillFeedTime => kill_feed_time: i64 = 6, KillFeed, 0.0, 3600.0, "seconds", "Lifetime of new kill-feed entries. Zero hides the feed. Shortening this also caps the lifetime of existing entries.";
    KillFeedLimit => kill_feed_limit: i64 = 5, KillFeed, 0.0, 100.0, "entries", "Maximum recent deaths shown in the feed. Zero hides the feed. Lowering this removes the oldest entries immediately.";
    WeaponSpawnChance => weapon_spawn_chance: f32 = 50.0, PickupSpawning, 0.0, 100.0, "%", "Chance of spawning a non-pistol weapon instead of a powerup at a map-authored pickup point.";
    ActivePickups => active_pickups: i64 = 8, PickupSpawning, 0.0, 24.0, "pickups", "Maximum active pickups. Initial spawns fill this many slots; respawns add one at a time.";
    PickupRespawn => pickup_respawn: f32 = 4.0, PickupSpawning, 0.0, 500.0, "seconds", "Time between attempts to replace a collected pickup. Zero attempts once each simulation tick.";
    PickupSpacing => pickup_spacing: f32 = 5.0, PickupSpawning, 0.0, 100.0, "units", "Minimum distance between active pickup positions.";
    PickupPlayerDistance => pickup_player_distance: f32 = 2.0, PickupSpawning, 0.0, 100.0, "units", "Minimum distance from players when a pickup appears.";
    PickupRadius => pickup_radius: f32 = 1.05, PickupSpawning, 0.0, 10.0, "units", "Player-center distance for collecting a pickup. Walls still block collection.";
 }
 presentation: PresentationTuning {
    DroneBlastViewTime => drone_blast_view_time: f32 = 1.25, Drone, 0.0, 30.0, "seconds", "Duration of the third-person view of your drone's explosion. Zero disables this camera; damage and respawn timing are independent.";
    CameraFov => camera_fov: f32 = 90.0, Camera, 10.0, 160.0, "degrees", "Vertical field of view for the world camera.";
    MouseSensitivity => mouse_sensitivity: f32 = 0.002, Camera, 0.0, 0.1, "radians/pixel", "Mouse turn sensitivity, applied to all players.";
    CameraRadius => camera_collision_radius: f32 = 0.15, Camera, 0.001, 3.0, "units", "Camera collision sphere used for presentation smoothing and the death camera.";
    DeathDistance => death_camera_distance: f32 = 4.0, DeathCamera, 0.0, 30.0, "units", "Distance pulled back during the existing death camera animation.";
    DeathHeight => death_camera_height: f32 = 1.0, DeathCamera, -10.0, 30.0, "units", "Vertical offset added during the existing death camera animation.";
    DeathEaseTime => death_camera_time: f32 = 2.0, DeathCamera, 0.0, 60.0, "seconds", "Time for the death camera to ease out. Zero moves to its final position immediately; respawn timing is independent.";
    WeaponFov => weapon_fov: f32 = 70.0, Viewmodel, 10.0, 160.0, "degrees", "Vertical FOV of the separate first-person weapon camera. Does not change world FOV or shot direction.";
    WeaponX => weapon_x: f32 = 0.23, Viewmodel, -3.0, 3.0, "units", "First-person weapon rest position on the horizontal axis.";
    WeaponY => weapon_y: f32 = -0.20, Viewmodel, -3.0, 3.0, "units", "First-person weapon rest position on the vertical axis.";
    WeaponZ => weapon_z: f32 = -0.46, Viewmodel, -3.0, -0.02, "units", "First-person weapon rest position on the depth axis. Negative values place it in front of the camera.";
    WeaponYaw => weapon_yaw: f32 = 0.08, Viewmodel, -3.14, 3.14, "radians", "Rest yaw of the first-person weapon.";
    WeaponPitch => weapon_pitch: f32 = 0.025, Viewmodel, -3.14, 3.14, "radians", "Rest pitch of the first-person weapon.";
    RecoilBack => recoil_back: f32 = 0.055, Recoil, -1.0, 1.0, "units", "Weapon movement toward the camera per shot. Cosmetic; does not affect aim.";
    RecoilUp => recoil_up: f32 = 0.012, Recoil, -1.0, 1.0, "units", "Upward weapon movement per shot. Cosmetic; does not affect aim.";
    RecoilPitch => recoil_pitch: f32 = 0.16, Recoil, -3.14, 3.14, "radians", "Weapon pitch kick per shot. Cosmetic; does not affect aim.";
    RecoilRoll => recoil_roll: f32 = -0.035, Recoil, -3.14, 3.14, "radians", "Weapon roll kick per shot. Cosmetic; does not affect aim.";
    RecoilReturn => recoil_return_speed: f32 = 18.0, Recoil, 0.0, 1000.0, "1/s", "Exponential recoil recovery rate. Zero holds the last shot kick.";
    MuzzleFlashTime => muzzle_flash_time: f32 = 0.07, Effects, 0.0, 10.0, "seconds", "Duration of the first-person muzzle flash for new shots.";
    MuzzleFlashSize => muzzle_flash_size: f32 = 1.0, Effects, 0.0, 10.0, "multiplier", "Size of the muzzle flash. Zero hides it.";
    HitMarkerTime => hit_marker_time: f32 = 0.12, Effects, 0.0, 10.0, "seconds", "Duration of the crosshair highlight after a server-confirmed hit.";
    BeamLength => beam_length: f32 = 2.4, Effects, 0.02, 100.0, "units", "Maximum length of the visible projectile trail. Does not affect collision.";
    BeamWidth => beam_width: f32 = 1.0, Effects, 0.0, 10.0, "multiplier", "Width multiplier for all projectile trail layers. Zero hides trails.";
    MuzzleBlend => muzzle_blend_distance: f32 = 4.0, Effects, 0.01, 100.0, "units", "Distance over which your projectile visual transitions from the weapon muzzle to its authoritative trajectory. Does not affect hits.";
    ImpactTime => impact_time: f32 = 0.3, Effects, 0.001, 10.0, "seconds", "Duration of impact splashes. Applies to existing splashes too.";
    ImpactSize => impact_size: f32 = 1.0, Effects, 0.0, 10.0, "multiplier", "Overall size and spread of impact splashes. Zero hides them.";
    ImpactRays => impact_rays: i64 = 8, Effects, 0.0, 64.0, "rays", "Number of rays in newly spawned impact splashes.";
    NameplateDistance => nameplate_distance: f32 = 45.0, Nameplates, 0.0, 1000.0, "units", "Maximum distance at which player labels are shown. Zero hides labels.";
    NameplateReference => nameplate_reference_distance: f32 = 10.0, Nameplates, 0.01, 1000.0, "units", "Distance at which nameplates appear at their base size. Farther labels scale down.";
    NameplateScale => nameplate_min_scale: f32 = 0.32, Nameplates, 0.0, 4.0, "multiplier", "Minimum label scale for distant players.";
    NameplateHeight => nameplate_height_offset: f32 = 0.35, Nameplates, -10.0, 10.0, "units", "Label anchor offset above the player capsule.";
    CrosshairSize => crosshair_size: f32 = 4.0, Crosshair, 0.0, 100.0, "pixels", "Size of the square crosshair. Zero hides it.";
    LightmapBrightness => lightmap_brightness: f32 = 1.0, Environment, 0.0, 10.0, "multiplier", "Brightness multiplier for map lighting, including vertex lighting and fullbright surfaces.";
    GlowBrightness => glow_brightness: f32 = 1.0, Environment, 0.0, 10.0, "multiplier", "Brightness multiplier for map glow textures.";
    AmbientBrightness => ambient_brightness: f32 = 300.0, Environment, 0.0, 10000.0, "illuminance", "Ambient illumination of dynamic players. Baked map lighting and first-person weapon lighting are independent.";
    WeaponBrightness => weapon_brightness: f32 = 450.0, Viewmodel, 0.0, 10000.0, "illuminance", "Ambient fill of the first-person weapon. Its two accent lights remain fixed.";
 }
}

impl SimulationTuning {
    pub fn scoped_fov_degrees(&self, world_fov_degrees: f32) -> f32 {
        // Optical magnification scales the projection plane, rather than
        // replacing a user's chosen FOV with a fixed angle.
        (2.0 * ((world_fov_degrees.to_radians() * 0.5).tan() / self.sniper_scope_zoom).atan())
            .to_degrees()
    }
}

impl Parameter {
    pub fn spec(self) -> &'static ParameterSpec {
        &PARAMETERS[self as usize]
    }
}

#[derive(Component, Resource, Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct FeatureSettings {
    pub revision: u64,
    pub values: SettingsValues,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Edit {
    Set(Parameter, Number),
    ResetParameter(Parameter),
    ResetCategory(Category),
    ResetAll,
}

impl FeatureSettings {
    pub fn apply(&mut self, revision: u64, edit: &Edit) -> Result<(), String> {
        if revision != self.revision {
            return Err("Settings changed on the server. Try again.".into());
        }
        let mut next = self.values;
        match *edit {
            Edit::Set(parameter, number) => next.set(parameter, number)?,
            Edit::ResetParameter(parameter) => next.set(parameter, parameter.spec().default)?,
            Edit::ResetCategory(category) => {
                for spec in PARAMETERS
                    .iter()
                    .filter(|spec| spec.category.is_within(category))
                {
                    next.set(spec.id, spec.default)?;
                }
            }
            Edit::ResetAll => next = SettingsValues::default(),
        }
        let sim = next.simulation;
        if sim.player_height < 2.0 * sim.player_radius {
            return Err("player_height must be at least twice player_radius.".into());
        }
        if sim.eye_height > sim.player_height {
            return Err("eye_height must not exceed player_height.".into());
        }
        if sim.shotgun_falloff_end <= sim.shotgun_falloff_start {
            return Err("shotgun_falloff_end must exceed shotgun_falloff_start.".into());
        }
        if sim.mg_max_spread < sim.mg_spread {
            return Err("mg_max_spread must be at least mg_spread.".into());
        }
        if next != self.values {
            self.values = next;
            self.revision += 1;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EditRequest {
    pub id: u64,
    pub revision: u64,
    pub edit: Edit,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EditReply {
    pub id: u64,
    pub settings: FeatureSettings,
    pub error: Option<String>,
}

pub fn seconds_to_ticks(seconds: f32) -> u16 {
    if seconds <= 0.0 {
        0
    } else {
        (seconds * crate::TICK_HZ as f32).round().max(1.0) as u16
    }
}
