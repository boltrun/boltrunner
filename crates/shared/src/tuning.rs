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
    World,
    Triggers,
    Spawning,
    Powerups,
    PickupEffects,
    PickupSpawning,
    Match,
    Bots,
    BotPerception,
    BotAim,
    BotCombat,
    BotNavigation,
    BotLoot,

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
        Self::World,
        Self::Triggers,
        Self::Spawning,
        Self::Powerups,
        Self::PickupEffects,
        Self::PickupSpawning,
        Self::Match,
        Self::Bots,
        Self::BotPerception,
        Self::BotAim,
        Self::BotCombat,
        Self::BotNavigation,
        Self::BotLoot,
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
            Self::World => "world",
            Self::Triggers => "triggers",
            Self::Spawning => "spawning",
            Self::Powerups => "powerups",
            Self::PickupEffects => "effects",
            Self::PickupSpawning => "spawning",
            Self::Match => "match",
            Self::Bots => "bots",
            Self::BotPerception => "perception",
            Self::BotAim => "aim",
            Self::BotCombat => "combat",
            Self::BotNavigation => "navigation",
            Self::BotLoot => "loot",

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
            Self::World => None,
            Self::Triggers => Some(Self::World),
            Self::Spawning => Some(Self::World),
            Self::Powerups => None,
            Self::PickupEffects => Some(Self::Powerups),
            Self::PickupSpawning => Some(Self::Powerups),
            Self::Match => None,
            Self::Bots => Some(Self::Player),
            Self::BotPerception => Some(Self::Bots),
            Self::BotAim => Some(Self::Bots),
            Self::BotCombat => Some(Self::Bots),
            Self::BotNavigation => Some(Self::Bots),
            Self::BotLoot => Some(Self::Bots),

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
                match parameter { $($(Parameter::$id => self.$group.$field = <$ty as TunableNumber>::from_number(value)?,)+)+ }
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
    PlayerHeight => player_height: f32 = 1.8, Body, 0.1, 10.0, "units", "Full capsule height, measured from the feet. Must be at least twice player_radius and no lower than eye_height.";
    EyeHeight => eye_height: f32 = 1.6, Body, 0.0, 10.0, "units", "Camera and shot origin height above the feet. Must not exceed player_height.";
    MaxHealth => max_health: i64 = 100, Health, 1.0, 65535.0, "HP", "Full health on spawn. Lowering this clamps current health; raising it does not heal existing players.";
    RespawnTime => respawn_time: f32 = 3.0, Health, 0.0, 60.0, "seconds", "Delay after death, rounded to simulation ticks. Zero respawns on the next tick. Existing deaths keep their original delay.";
    FireInterval => fire_interval: f32 = 0.2, Pistol, 0.0, 60.0, "seconds", "Minimum interval between automatic shots, rounded to simulation ticks. Zero fires once per simulation tick. Existing cooldowns keep their duration.";
    ProjectileDamage => projectile_damage: i64 = 25, Projectiles, 0.0, 65535.0, "HP", "Damage per confirmed player hit. New shots capture this value; existing projectiles keep their damage.";
    ProjectileSpeed => projectile_speed: f32 = 120.0, Projectiles, 0.0, 1000.0, "units/s", "Projectile travel speed. New shots capture this value.";
    ProjectileRadius => projectile_radius: f32 = 0.045, Projectiles, 0.001, 3.0, "units", "Collision sphere radius for both walls and players. New shots capture this value; visual beam width is independent.";
    ProjectileLifetime => projectile_lifetime: f32 = 2.0, Projectiles, 0.001, 60.0, "seconds", "Maximum projectile lifetime, rounded to simulation ticks. New shots capture this value. Range is speed multiplied by lifetime.";
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
    ActivePickups => active_pickups: i64 = 8, PickupSpawning, 0.0, 24.0, "pickups", "Maximum active pickups. Initial spawns fill this many slots; respawns add one at a time.";
    PickupRespawn => pickup_respawn: f32 = 4.0, PickupSpawning, 0.0, 500.0, "seconds", "Time between attempts to replace a collected pickup. Zero attempts once each simulation tick.";
    PickupSpacing => pickup_spacing: f32 = 5.0, PickupSpawning, 0.0, 100.0, "units", "Minimum distance between active pickup positions.";
    PickupPlayerDistance => pickup_player_distance: f32 = 2.0, PickupSpawning, 0.0, 100.0, "units", "Minimum distance from players when a pickup appears.";
    PickupRadius => pickup_radius: f32 = 1.05, PickupSpawning, 0.0, 10.0, "units", "Player-center distance for collecting a pickup. Walls still block collection.";
 }
 bots: BotTuning {
    BotCount => bot_count: i64 = crate::player_color::PALETTE.len() as i64, Bots, 0.0, crate::player_color::PALETTE.len() as f64, "bots", "Requested server-controlled opponents. Defaults to filling every available place, limited by server capacity and free palette colors. Zero disables bots. Humans always take priority over bots for colors and places.";
    BotReactionTime => bot_reaction_time: f32 = 0.49, BotPerception, 0.0, 5.0, "seconds", "Visible time required before firing at a newly acquired target. Evaluated at the decision interval.";
    BotDetectionRange => bot_detection_range: f32 = 55.0, BotPerception, 0.0, 500.0, "units", "Maximum distance for detecting visible opponents. Walls always block detection.";
    BotDetectionFov => bot_detection_fov: f32 = 197.25385, BotPerception, 0.0, 360.0, "degrees", "Full field of view centered on the current look direction. 360 detects opponents in every direction.";
    BotTargetMemory => bot_target_memory: f32 = 3.0, BotPerception, 0.0, 30.0, "seconds", "Time after last sighting before reacquisition requires a fresh reaction delay. Does not reveal hidden positions.";
    BotAimError => bot_aim_error: f32 = 1.776169, BotAim, 0.0, 45.0, "degrees", "Maximum horizontal and vertical aim oscillation. Zero removes deliberate error.";
    BotTurnSpeed => bot_turn_speed: f32 = 158.13635, BotAim, 0.0, 1440.0, "degrees/s", "Maximum yaw and pitch tracking speed; zero stops turning.";
    BotFireTolerance => bot_fire_tolerance: f32 = 4.583662, BotAim, 0.0, 90.0, "degrees", "Maximum remaining yaw and pitch aiming error at which firing is allowed.";
    BotAimHeight => bot_aim_height: f32 = 0.65, BotAim, 0.0, 1.0, "fraction", "Aim point above target feet as a fraction of target capsule height.";
    BotLeadFactor => bot_lead_factor: f32 = 0.65, BotAim, 0.0, 2.0, "multiplier", "Fraction of visible target velocity used to lead projectile shots.";
    BotLeadTime => bot_lead_time: f32 = 0.4, BotAim, 0.0, 3.0, "seconds", "Maximum projectile flight time considered for leading the target.";
    BotAimYawFrequency => bot_aim_yaw_frequency: f32 = 4.3, BotAim, 0.0, 30.0, "radians/s", "Horizontal oscillation speed of deliberate aiming error.";
    BotAimPitchFrequency => bot_aim_pitch_frequency: f32 = 3.1, BotAim, 0.0, 30.0, "radians/s", "Vertical oscillation speed of deliberate aiming error.";
    BotCombatDistance => bot_combat_distance: f32 = 14.0, BotCombat, 0.0, 100.0, "units", "Approach the opponent when farther away than this distance.";
    BotRetreatDistance => bot_retreat_distance: f32 = 5.0, BotCombat, 0.0, 100.0, "units", "Back away when the opponent is nearer than this distance. Must not exceed combat_distance.";
    BotStrafeWeight => bot_strafe_weight: f32 = 0.8, BotCombat, 0.0, 5.0, "weight", "Sideways movement weight before normalizing movement direction. Zero disables strafing.";
    BotApproachWeight => bot_approach_weight: f32 = 0.6, BotCombat, 0.0, 5.0, "weight", "Weight of forward and retreat movement relative to strafing.";
    BotStrafeInterval => bot_strafe_interval: f32 = 1.5, BotCombat, 0.05, 30.0, "seconds", "Time between alternating strafe directions, staggered between bots.";
    BotEvasionInterval => bot_evasion_interval: f32 = 3.0, BotCombat, 0.0, 60.0, "seconds", "Minimum time between combat jump/dash attempts. Zero disables both.";
    BotJumpChance => bot_jump_chance: f32 = 0.5, BotCombat, 0.0, 1.0, "probability", "Fraction of safe combat evasion attempts that jump instead of dashing. Zero always attempts dash; one always jumps.";
    BotSafetyDistance => bot_safety_distance: f32 = 1.6, BotNavigation, 0.1, 10.0, "units", "Forward ground and obstruction probe length for combat steering. Does not change physical player movement speed.";
    BotThinkInterval => bot_think_interval: f32 = 0.125, BotNavigation, 0.05, 2.0, "seconds", "Interval between expensive visibility and tactical decisions. Steering remains at 120 Hz. Minimum bounds server work.";
    BotRepathInterval => bot_repath_interval: f32 = 1.0, BotNavigation, 0.1, 30.0, "seconds", "Minimum interval for refreshing pickup route priorities. Invalidated targets can be replaced immediately.";
    BotWaypointRadius => bot_waypoint_radius: f32 = 0.75, BotNavigation, 0.05, 1.5, "units", "Distance at which a navigation waypoint counts as reached.";
    BotLootStopRadius => bot_loot_stop_radius: f32 = 0.3, BotNavigation, 0.01, 2.0, "units", "Distance from a reachable pickup goal at which direct approach stops.";
    BotStuckInterval => bot_stuck_interval: f32 = 1.0, BotNavigation, 0.1, 10.0, "seconds", "Time between checks for failure to move.";
    BotStuckDistance => bot_stuck_distance: f32 = 0.3, BotNavigation, 0.01, 3.0, "units", "Minimum displacement during a stuck check interval.";
    BotStuckJumpAttempts => bot_stuck_jump_attempts: i64 = 2, BotNavigation, 0.0, 10.0, "attempts", "Number of consecutive stuck checks allowed to attempt an escape jump.";
    BotIdleTurn => bot_idle_turn: f32 = 11.459156, BotNavigation, 0.0, 180.0, "degrees/decision", "Look-around angle per decision when no reachable route or pickup is available.";
    BotPatrolDistance => bot_patrol_distance: f32 = 3.0, BotNavigation, 0.0, 50.0, "units", "Minimum distance for choosing a patrol destination among reachable navigation nodes.";
    BotLootRange => bot_loot_range: f32 = 35.0, BotLoot, 0.0, 300.0, "units", "Search radius for reachable powerups and optional equipment offers.";
    BotCombatPriorityRange => bot_combat_priority_range: f32 = 12.0, BotLoot, 0.0, 100.0, "units", "A visible opponent within this radius takes priority over travelling to any pickup.";
    BotUrgentHealth => bot_urgent_health: f32 = 0.35, BotLoot, 0.0, 1.0, "fraction", "Health fraction below which emergency pickup reevaluation is requested.";
    BotUrgentUtility => bot_urgent_utility: f32 = 70.0, BotLoot, 0.0, 1000.0, "utility", "Pickup utility that can justify seeking loot during distant combat.";
    BotUsefulUtility => bot_useful_utility: f32 = 40.0, BotLoot, 0.0, 1000.0, "utility", "Utility needed for a nearby pickup to justify a short detour during distant combat.";
    BotNearbyLootRange => bot_nearby_loot_range: f32 = 6.0, BotLoot, 0.0, 50.0, "units", "Distance for a useful pickup to qualify as a short combat detour.";
    BotImmediateLootRange => bot_immediate_loot_range: f32 = 3.0, BotLoot, 0.0, 50.0, "units", "Distance at which any desired pickup may be approached during distant combat.";
    BotLootCommitment => bot_loot_commitment: f32 = 1.6, BotLoot, 1.0, 10.0, "multiplier", "Preference for retaining the current pickup goal instead of oscillating between similar goals.";
    BotLootDistanceScale => bot_loot_distance_scale: f32 = 10.0, BotLoot, 0.1, 100.0, "units", "Distance scale for reducing pickup priority with travel distance. Larger values favor distant valuable loot.";
    BotHealthMissing => bot_health_missing: f32 = 0.05, BotLoot, 0.0, 1.0, "fraction", "Minimum missing health before a bot deliberately seeks a health pickup.";
    BotHealthUtility => bot_health_utility: f32 = 20.0, BotLoot, 0.0, 1000.0, "utility", "Base utility of a useful health pickup.";
    BotMissingHealthUtility => bot_missing_health_utility: f32 = 100.0, BotLoot, 0.0, 1000.0, "utility", "Additional health pickup utility multiplied by missing health fraction.";
    BotBuffRefreshTime => bot_buff_refresh_time: f32 = 1.0, BotLoot, 0.0, 60.0, "seconds", "Remaining speed/rapid-fire duration below which a bot seeks a refresh.";
    BotSpeedUtility => bot_speed_utility: f32 = 25.0, BotLoot, 0.0, 1000.0, "utility", "Priority of a speed pickup when a refresh is needed. Zero disables seeking it.";
    BotRapidFireUtility => bot_rapid_fire_utility: f32 = 30.0, BotLoot, 0.0, 1000.0, "utility", "Priority of a rapid-fire pickup when a refresh is needed. Zero disables seeking it.";
 }
 presentation: PresentationTuning {
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
        if next.bots.bot_retreat_distance > next.bots.bot_combat_distance {
            return Err("bot_retreat_distance must not exceed bot_combat_distance.".into());
        }
        let sim = next.simulation;
        if sim.player_height < 2.0 * sim.player_radius {
            return Err("player_height must be at least twice player_radius.".into());
        }
        if sim.eye_height > sim.player_height {
            return Err("eye_height must not exceed player_height.".into());
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
