use bevy::prelude::*;
use boltrunner_shared::{
    PlayerId, PlayerInput, PlayerState, TICK_DURATION, arena, level,
    match_state::MatchState,
    player_color::{PALETTE, PlayerColor},
    powerups::{Pickup, PickupKind},
    protocol::{BotPlayer, PlayerName},
    tuning::{BotTuning, FeatureSettings, SimulationTuning},
};
use lightyear::prelude::{input::native::ActionState, *};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::{Duration, Instant},
};

const CELL: f32 = 1.75;
const MAX_NODES: usize = 4096;
const MAX_SEARCH: usize = 512;
const NAMES: [&str; 8] = [
    "Bot Astra",
    "Bot Comet",
    "Bot Pixel",
    "Bot Nova",
    "Bot Orion",
    "Bot Vega",
    "Bot Echo",
    "Bot Pulse",
];

fn available_name(used: &mut HashSet<String>, serial: usize) -> String {
    for offset in 0..NAMES.len() {
        let name = NAMES[serial.wrapping_add(offset) % NAMES.len()];
        if used.insert(name.into()) {
            return name.into();
        }
    }
    let mut suffix = serial;
    loop {
        let name = format!("Bot{suffix:x}");
        if used.insert(name.clone()) {
            return name;
        }
        suffix = suffix.wrapping_add(1);
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BotSystems {
    Offers,
    Decide,
    Equipment,
}

pub struct BotsPlugin;
impl Plugin for BotsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Director>()
            .init_resource::<BotLootTargets>()
            .add_systems(Update, balance.after(crate::spawn_players))
            .configure_sets(
                FixedUpdate,
                (
                    BotSystems::Offers,
                    BotSystems::Decide,
                    BotSystems::Equipment,
                )
                    .chain()
                    .before(crate::history::capture_before),
            )
            .add_systems(FixedUpdate, think.in_set(BotSystems::Decide));
    }
}

#[derive(Component)]
struct Brain {
    loot: Option<LootGoal>,
    seeking_loot: bool,
    emergency_at: u64,
    serial: usize,
    next_think: u64,
    path: VecDeque<usize>,
    repath_at: u64,
    target: Option<u64>,
    seen_at: u64,
    last_seen: u64,
    last_evasion: u64,
    target_yaw: f32,
    target_pitch: f32,
    fire: bool,
    movement: Vec2,
    last_position: Vec3,
    stuck_at: u64,
    stuck: u8,
    relocation: u16,
}

/// Optional server adapters can offer collectibles without coupling the bot brain
/// to a particular weapon inventory or replicating AI-only data to clients.
#[derive(Resource, Default)]
pub struct BotLootTargets(pub Vec<BotLootTarget>);
#[derive(Clone, Copy)]
pub struct BotLootTarget {
    pub collector: Entity,
    pub target: Entity,
    pub position: Vec3,
    pub utility: f32,
}
#[derive(Component, Default)]
pub struct BotLootIntent {
    pub target: Option<Entity>,
}
#[derive(Clone, Copy)]
struct LootGoal {
    target: Entity,
    position: Vec3,
    utility: f32,
}

#[derive(Resource, Default)]
struct Director {
    tick: u64,
    graph: Graph,
    key: Option<(String, [f32; 7])>,
    serial: usize,
}

fn balance(
    mut commands: Commands,
    players: Query<(Entity, &PlayerState, &PlayerColor, Option<&Brain>), With<PlayerId>>,
    settings: Res<FeatureSettings>,
    round: Res<MatchState>,
    mut director: ResMut<Director>,
    names: Query<&PlayerName>,
) {
    let humans = players
        .iter()
        .filter(|(_, _, _, brain)| brain.is_none())
        .count();
    let mut human_colors = [false; PALETTE.len()];
    for (_, _, color, brain) in &players {
        if brain.is_none() {
            human_colors[color.0 as usize] = true;
        }
    }
    let desired = (settings.values.bots.bot_count.max(0) as usize)
        .min(arena::MAX_PLAYERS.saturating_sub(humans))
        .min(human_colors.iter().filter(|used| !**used).count());
    let bots: Vec<_> = players.iter().filter(|(_, _, _, b)| b.is_some()).collect();
    for (entity, _, _, _) in bots.iter().skip(desired) {
        commands.entity(*entity).despawn();
    }
    if bots.len() >= desired {
        return;
    }
    let mut used_names: HashSet<_> = names.iter().map(|name| name.0.clone()).collect();
    // Departed scores stay visible until the round ends.
    used_names.extend(round.rows.iter().map(|row| row.nickname.clone()));
    let mut colors = [false; PALETTE.len()];
    let mut occupied = Vec::new();
    for (entity, state, color, brain) in &players {
        if brain.is_none()
            || bots
                .iter()
                .take(desired)
                .any(|(retained, _, _, _)| *retained == entity)
        {
            colors[color.0 as usize] = true;
            occupied.push(state.position);
        }
    }
    for _ in bots.len()..desired {
        let Some(color) = colors.iter().rposition(|used| !used) else {
            break;
        };
        let sim = settings.values.simulation;
        let index = arena::choose_spawn(
            rand::random::<u32>() as usize,
            &occupied,
            sim.spawn_separation,
        );
        let spawn = arena::spawn(index, sim);
        let serial = director.serial;
        director.serial = director.serial.wrapping_add(1);
        let name = available_name(&mut used_names, serial);
        let id = rand::random::<u64>();
        let state = PlayerState {
            tuning: sim,
            position: spawn.position,
            yaw: spawn.yaw,
            view_yaw_offset: spawn.yaw,
            spawn_index: index as u8,
            health: boltrunner_shared::health::Health(sim.max_health as u16),
            jump: boltrunner_shared::protocol::JumpState {
                air_jumps_remaining: sim.air_jumps as u8,
                ..default()
            },
            dash: boltrunner_shared::protocol::DashState::new(sim),
            grounded: true,
            match_paused: round.results,
            ..default()
        };
        commands.spawn((
            PlayerId(id),
            PlayerName(name.clone()),
            PlayerColor(color as u8),
            BotPlayer,
            state,
            ActionState::<PlayerInput>::default(),
            BotLootIntent::default(),
            Brain {
                loot: None,
                seeking_loot: false,
                emergency_at: 0,
                serial,
                next_think: director.tick
                    + serial as u64 % ticks(settings.values.bots.bot_think_interval),
                path: default(),
                repath_at: 0,
                target: None,
                seen_at: 0,
                last_seen: 0,
                last_evasion: director.tick,
                target_yaw: spawn.yaw,
                target_pitch: 0.0,
                fire: false,
                movement: Vec2::ZERO,
                last_position: spawn.position,
                stuck_at: director.tick + ticks(settings.values.bots.bot_stuck_interval),
                stuck: 0,
                relocation: 0,
            },
            Replicate::to_clients(NetworkTarget::All),
            InterpolationTarget::to_clients(NetworkTarget::All),
        ));
        colors[color] = true;
        occupied.push(spawn.position);
        info!("Spawned {name} ({id:016x})");
    }
}

#[derive(Clone, Copy)]
struct Actor {
    id: u64,
    position: Vec3,
    velocity: Vec3,
    height: f32,
}

fn think(
    mut director: ResMut<Director>,
    settings: Res<FeatureSettings>,
    round: Res<MatchState>,
    actors: Query<(&PlayerId, &PlayerState)>,
    pickups: Query<(Entity, &Pickup)>,
    offers: Res<BotLootTargets>,
    mut bots: Query<(
        Entity,
        &PlayerState,
        &mut Brain,
        &mut BotLootIntent,
        &mut ActionState<PlayerInput>,
    )>,
) {
    if bots.is_empty() {
        return;
    }
    director.tick += 1;
    let tick = director.tick;
    let sim = settings.values.simulation;
    let key = (
        round.map.clone(),
        [
            sim.player_radius,
            sim.player_height,
            sim.step_height,
            sim.walkable_normal_y,
            sim.kill_plane_height,
            sim.ground_snap_distance,
            sim.ground_probe_distance,
        ],
    );
    if director.key.as_ref() != Some(&key) {
        director.key = Some(key);
        director.graph = Graph::new(sim);
        for (_, _, mut brain, mut intent, _) in &mut bots {
            brain.path.clear();
            brain.loot = None;
            intent.target = None;
            brain.repath_at = 0;
        }
    }
    if round.results {
        for (_, _, mut brain, mut intent, mut input) in &mut bots {
            brain.fire = false;
            intent.target = None;
            input.0.fire = false;
        }
        return;
    }
    // A map/tuning change rebuilds one shared graph, with a hard per-tick work cap.
    director.graph.grow(sim, Duration::from_micros(400));
    let decision_due =
        settings.is_changed() || bots.iter().any(|(_, _, b, _, _)| tick >= b.next_think);
    let targets: Vec<_> = if decision_due {
        actors
            .iter()
            .filter_map(|(id, state)| {
                (state.death.is_none() && !state.match_paused).then_some(Actor {
                    id: id.0,
                    position: state.position,
                    velocity: Vec3::new(
                        state.velocity.x,
                        state.vertical_velocity,
                        state.velocity.y,
                    ),
                    height: state.tuning.player_height,
                })
            })
            .collect()
    } else {
        Vec::new()
    };
    let tuning = settings.values.bots;
    let think_ticks = ticks(tuning.bot_think_interval);
    for (entity, state, mut brain, mut intent, mut input) in &mut bots {
        if state.death.is_some() || state.match_paused {
            brain.path.clear();
            brain.target = None;
            brain.loot = None;
            intent.target = None;
            brain.fire = false;
            input.0.fire = false;
            input.0.forward = false;
            input.0.backward = false;
            input.0.left = false;
            input.0.right = false;
            continue;
        }
        if brain.relocation != state.relocation {
            brain.loot = None;
            brain.relocation = state.relocation;
            brain.path.clear();
            brain.repath_at = 0;
            brain.stuck = 0;
            brain.target_yaw = state.yaw;
            brain.last_position = state.position;
        }
        if settings.is_changed() {
            brain.next_think = brain
                .next_think
                .min(tick + brain.serial as u64 % think_ticks);
            brain.repath_at = brain.repath_at.min(tick);
            brain.stuck_at = brain.stuck_at.min(tick + ticks(tuning.bot_stuck_interval));
        }
        if tick >= brain.next_think {
            brain.next_think = tick + think_ticks;
            decide(
                &director.graph,
                tick,
                sim,
                tuning,
                state,
                &targets,
                entity,
                &pickups,
                &offers,
                &mut brain,
                &mut input.0,
            );
        }
        intent.target = brain
            .loot
            .filter(|_| brain.seeking_loot)
            .map(|goal| goal.target);
        // Only cheap steering/aim smoothing runs at simulation frequency.
        while brain.path.front().is_some_and(|&i| {
            state
                .position
                .distance_squared(director.graph.nodes[i].position)
                < tuning.bot_waypoint_radius.powi(2)
        }) {
            brain.path.pop_front();
        }
        let desired = brain
            .path
            .front()
            .map(|&i| {
                (director.graph.nodes[i].position - state.position)
                    .xz()
                    .normalize_or_zero()
            })
            .unwrap_or_else(|| {
                brain
                    .loot
                    .filter(|_| brain.seeking_loot)
                    .map_or(brain.movement, |goal| {
                        let delta = goal.position - state.position;
                        if delta.xz().length_squared() < tuning.bot_loot_stop_radius.powi(2) {
                            Vec2::ZERO
                        } else {
                            delta.xz().normalize_or_zero()
                        }
                    })
            });
        let max_turn = tuning.bot_turn_speed.to_radians() * TICK_DURATION.as_secs_f32();
        let aim = state.yaw + angle_delta(state.yaw, brain.target_yaw).clamp(-max_turn, max_turn);
        input.0.yaw = PlayerInput::encode_yaw(aim - state.view_yaw_offset);
        let pitch = input.0.pitch_radians()
            + (brain.target_pitch - input.0.pitch_radians()).clamp(-max_turn, max_turn);
        input.0.pitch = PlayerInput::encode_pitch(pitch);
        input.0.fire = brain.fire
            && angle_delta(aim, brain.target_yaw).abs() <= tuning.bot_fire_tolerance.to_radians()
            && (pitch - brain.target_pitch).abs() <= tuning.bot_fire_tolerance.to_radians();
        let (sin, cos) = aim.sin_cos();
        let local = Vec2::new(
            desired.dot(Vec2::new(cos, -sin)),
            desired.dot(Vec2::new(-sin, -cos)),
        );
        input.0.right = local.x > 0.35;
        input.0.left = local.x < -0.35;
        input.0.forward = local.y > 0.35;
        input.0.backward = local.y < -0.35;
    }
}

fn ticks(seconds: f32) -> u64 {
    (seconds / TICK_DURATION.as_secs_f32()).ceil().max(1.0) as u64
}

fn angle_delta(from: f32, to: f32) -> f32 {
    (to - from + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

fn decide(
    graph: &Graph,
    tick: u64,
    sim: SimulationTuning,
    tuning: BotTuning,
    state: &PlayerState,
    actors: &[Actor],
    entity: Entity,
    pickups: &Query<(Entity, &Pickup)>,
    offers: &BotLootTargets,
    brain: &mut Brain,
    input: &mut PlayerInput,
) {
    let eye = state.position + Vec3::Y * sim.eye_height;
    let own = state.position;
    let mut candidates: Vec<_> = actors
        .iter()
        .filter(|a| {
            a.position.distance_squared(own) > 0.001
                && a.position.distance_squared(own) < tuning.bot_detection_range.powi(2)
        })
        .collect();
    candidates.sort_by(|a, b| {
        a.position
            .distance_squared(own)
            .total_cmp(&b.position.distance_squared(own))
    });
    let visible = candidates.into_iter().take(4).find(|actor| {
        let offset = actor.position + Vec3::Y * actor.height * tuning.bot_aim_height - eye;
        let forward = Vec3::new(-state.yaw.sin(), 0.0, -state.yaw.cos());
        forward.dot(offset.normalize_or_zero())
            >= (tuning.bot_detection_fov.to_radians() * 0.5).cos()
            && level::world().sweep_sphere(eye, offset, 0.02).is_none()
    });
    let emergency = tick >= brain.emergency_at
        && (state.health.0 as f32) < state.tuning.max_health as f32 * tuning.bot_urgent_health
        && brain
            .loot
            .is_none_or(|goal| goal.utility < tuning.bot_urgent_utility);
    if tick >= brain.repath_at
        || (visible.is_none() && brain.loot.is_some() && !brain.seeking_loot)
        || emergency
        || brain.loot.is_some_and(|goal| {
            pickups.get(goal.target).is_err()
                && !offers
                    .0
                    .iter()
                    .take(256)
                    .any(|offer| offer.collector == entity && offer.target == goal.target)
        })
    {
        brain.repath_at = tick + ticks(tuning.bot_repath_interval);
        brain.emergency_at = tick + ticks(tuning.bot_repath_interval);
        choose_loot(graph, entity, state, tuning, pickups, offers, brain);
    }
    // Nearby visible opponents keep combat priority, even at low health.
    // Loot at our feet is still collected by the ordinary pickup systems.
    let close_enemy = visible.is_some_and(|target| {
        target.position.distance_squared(own) < tuning.bot_combat_priority_range.powi(2)
    });
    let seek_loot = !close_enemy
        && brain.loot.is_some_and(|goal| {
            visible.is_none()
                || goal.utility >= tuning.bot_urgent_utility
                || (goal.utility >= tuning.bot_useful_utility
                    && goal.position.distance_squared(own) < tuning.bot_nearby_loot_range.powi(2))
                || goal.position.distance_squared(own) < tuning.bot_immediate_loot_range.powi(2)
        });
    brain.seeking_loot = seek_loot;
    if let Some(target) = visible {
        if !seek_loot {
            brain.path.clear();
        }
        if brain.target != Some(target.id) {
            brain.seen_at = tick;
            brain.target = Some(target.id);
        }
        brain.last_seen = tick;
        let travel = (target.position.distance(own) / sim.projectile_speed.max(1.0))
            .min(tuning.bot_lead_time);
        let aim = target.position
            + Vec3::Y * target.height * tuning.bot_aim_height
            + target.velocity * travel * tuning.bot_lead_factor
            - eye;
        // Continuous, id-seeded error, with directly editable amplitude and frequency.
        let phase = tick as f32 * TICK_DURATION.as_secs_f32() + brain.serial as f32 * 2.17;
        let error = tuning.bot_aim_error.to_radians();
        brain.target_yaw =
            (-aim.x).atan2(-aim.z) + (phase * tuning.bot_aim_yaw_frequency).sin() * error;
        brain.target_pitch = (aim.y / aim.length().max(0.001)).asin()
            + (phase * tuning.bot_aim_pitch_frequency).cos() * error;
        brain.fire = tick.saturating_sub(brain.seen_at)
            >= (tuning.bot_reaction_time / TICK_DURATION.as_secs_f32()).ceil() as u64;
        let distance = target.position.distance(own);
        let toward = (target.position - own).xz().normalize_or_zero();
        let side = Vec2::new(-toward.y, toward.x)
            * if (tick / ticks(tuning.bot_strafe_interval) + brain.serial as u64) % 2 == 0 {
                1.0
            } else {
                -1.0
            };
        let movement = (side * tuning.bot_strafe_weight
            + toward
                * if distance > tuning.bot_combat_distance {
                    tuning.bot_approach_weight
                } else if distance < tuning.bot_retreat_distance {
                    -tuning.bot_approach_weight
                } else {
                    0.0
                })
        .normalize_or_zero();
        if seek_loot {
            brain.movement = Vec2::ZERO;
        } else if safe_step(own, movement, sim, tuning.bot_safety_distance) {
            brain.path.clear();
            brain.movement = movement;
        } else if safe_step(own, -movement, sim, tuning.bot_safety_distance) {
            brain.path.clear();
            brain.movement = -movement;
        } else {
            brain.movement = Vec2::ZERO;
        }
        // Occasional jumps/dashes on known supported space, never boosted stats.
        if !seek_loot
            && tuning.bot_evasion_interval > 0.0
            && tick.saturating_sub(brain.last_evasion) >= ticks(tuning.bot_evasion_interval)
            && state.grounded
            && safe_step(own, movement * 2.0, sim, tuning.bot_safety_distance)
        {
            brain.last_evasion = tick;
            // Stable per-attempt sampling keeps decisions reproducible without shared RNG.
            let sample = (brain.serial as u64)
                .wrapping_mul(6364136223846793005)
                .wrapping_add(tick.wrapping_mul(1442695040888963407));
            if (sample >> 40) as f32 / ((1u32 << 24) as f32) < tuning.bot_jump_chance {
                input.jump_press = input.jump_press.wrapping_add(1);
            } else if state.dash.charges > 0 {
                input.dash_press = input.dash_press.wrapping_add(1);
            }
        }
    } else {
        brain.fire = false;
        // No hidden-player position is used for pursuit: patrol authored safe space.
        if brain.loot.is_none() && brain.path.is_empty() {
            if let Some(start) = graph.nearest(own) {
                brain.path = graph.patrol(
                    start,
                    tuning.bot_patrol_distance,
                    brain
                        .serial
                        .wrapping_mul(977)
                        .wrapping_add((tick / 120) as usize)
                        .wrapping_add(brain.stuck as usize * 317),
                );
            }
        }
        brain.movement = Vec2::ZERO;
        if let Some(&i) = brain.path.front() {
            let direction = graph.nodes[i].position - own;
            brain.target_yaw = (-direction.x).atan2(-direction.z);
            brain.target_pitch = 0.0;
        } else {
            // Look around while an isolated spawn's safe graph is still growing.
            let delta = brain.loot.map(|goal| goal.position - own);
            brain.target_yaw = delta.map_or(state.yaw + tuning.bot_idle_turn.to_radians(), |d| {
                (-d.x).atan2(-d.z)
            });
            brain.target_pitch = 0.0;
        }
        if brain.target.is_some()
            && tick.saturating_sub(brain.last_seen) >= ticks(tuning.bot_target_memory)
        {
            brain.target = None;
        }
    }
    if tick >= brain.stuck_at {
        brain.stuck_at = tick + ticks(tuning.bot_stuck_interval);
        let trying = !brain.path.is_empty() || brain.movement != Vec2::ZERO;
        if trying && brain.last_position.distance_squared(own) < tuning.bot_stuck_distance.powi(2) {
            brain.stuck = brain.stuck.saturating_add(1);
            brain.path.clear();
            brain.loot = None;
            brain.repath_at = 0;
            // Jump a low obstruction; a new patrol goal prevents repeated wall pushing.
            if state.grounded && brain.stuck as i64 <= tuning.bot_stuck_jump_attempts {
                input.jump_press = input.jump_press.wrapping_add(1);
            }
            brain.movement = -Vec2::new(-state.yaw.sin(), -state.yaw.cos());
        } else {
            brain.stuck = 0;
        }
        brain.last_position = own;
    }
}

fn pickup_utility(state: &PlayerState, kind: PickupKind, tuning: BotTuning) -> f32 {
    match kind {
        PickupKind::Health => {
            let missing = 1.0 - state.health.0 as f32 / state.tuning.max_health.max(1) as f32;
            if missing > tuning.bot_health_missing {
                tuning.bot_health_utility + tuning.bot_missing_health_utility * missing
            } else {
                0.0
            }
        }
        PickupKind::Speed => {
            if (state.speed_ticks as u64) < ticks(tuning.bot_buff_refresh_time) {
                tuning.bot_speed_utility
            } else {
                0.0
            }
        }
        PickupKind::RapidFire => {
            if (state.rapid_fire_ticks as u64) < ticks(tuning.bot_buff_refresh_time) {
                tuning.bot_rapid_fire_utility
            } else {
                0.0
            }
        }
    }
}

fn choose_loot(
    graph: &Graph,
    entity: Entity,
    state: &PlayerState,
    tuning: BotTuning,
    pickups: &Query<(Entity, &Pickup)>,
    offers: &BotLootTargets,
    brain: &mut Brain,
) {
    let Some(start) = graph.nearest(state.position) else {
        return;
    };
    // One bounded traversal serves every candidate; unreachable floors are never
    // accepted merely because their nodes are geometrically close.
    let previous = graph.reachable(start);
    let mut candidates: Vec<LootGoal> = pickups
        .iter()
        .take(64)
        .map(|(target, pickup)| LootGoal {
            target,
            position: pickup.position,
            utility: pickup_utility(state, pickup.kind, tuning),
        })
        .chain(
            offers
                .0
                .iter()
                .take(256)
                .filter(|offer| offer.collector == entity)
                .map(|offer| LootGoal {
                    target: offer.target,
                    position: offer.position,
                    utility: offer.utility,
                }),
        )
        .filter(|goal| {
            goal.utility > 0.0
                && goal.position.distance_squared(state.position) < tuning.bot_loot_range.powi(2)
        })
        .collect();
    candidates.sort_by(|a, b| {
        let score = |goal: &LootGoal| {
            let loyalty = if brain.loot.is_some_and(|old| old.target == goal.target) {
                tuning.bot_loot_commitment
            } else {
                1.0
            };
            goal.utility * loyalty
                / (1.0 + goal.position.distance(state.position) / tuning.bot_loot_distance_scale)
        };
        score(b)
            .total_cmp(&score(a))
            .then(a.target.to_bits().cmp(&b.target.to_bits()))
    });
    for mut goal in candidates.into_iter().take(8) {
        let world = level::world().character(state.tuning);
        let Some(floor) = world.ground(goal.position + Vec3::Y * 0.5, 2.0) else {
            continue;
        };
        if !safe_position(floor, state.tuning) {
            continue;
        }
        let Some(end) = graph.nearest(floor) else {
            continue;
        };
        let node = graph.nodes[end].position;
        if previous[end] == usize::MAX
            || node.distance_squared(floor) > CELL.powi(2)
            || !walk_to(node, floor, state.tuning)
            || level::world()
                .sweep_sphere(
                    node + Vec3::Y * state.tuning.player_height * 0.5,
                    goal.position - (node + Vec3::Y * state.tuning.player_height * 0.5),
                    0.02,
                )
                .is_some()
        {
            continue;
        }
        goal.position = floor;
        brain.path = Graph::route(&previous, start, end);
        brain.loot = Some(goal);
        return;
    }
    if brain.loot.is_some() {
        brain.path.clear();
    }
    brain.loot = None;
}

fn walk_to(mut position: Vec3, target: Vec3, tuning: SimulationTuning) -> bool {
    for _ in 0..8 {
        let Some(next) = walk_sample(position, target.xz(), tuning) else {
            return false;
        };
        position = next;
        if position.distance_squared(target) < 0.2_f32.powi(2) {
            return true;
        }
    }
    false
}

fn safe_position(position: Vec3, tuning: SimulationTuning) -> bool {
    position.y > tuning.kill_plane_height + 1.0
        && !level::data().triggers.iter().any(|trigger| {
            matches!(trigger.kind, level::TriggerKind::Hurt) && trigger.touches(position, &tuning)
        })
}

fn safe_step(position: Vec3, direction: Vec2, tuning: SimulationTuning, distance: f32) -> bool {
    let world = level::world().character(tuning);
    let delta = Vec3::new(direction.x, 0.0, direction.y) * distance;
    if world.sweep(position + Vec3::Y * 0.04, delta).is_some() {
        return false;
    }
    (1..=4).all(|sample| {
        world
            .ground(
                position + delta * (sample as f32 / 4.0) + Vec3::Y * 0.1,
                0.25,
            )
            .is_some_and(|floor| safe_position(floor, tuning))
    })
}

// Validate edges using the same capsule/stair movement as real players.
// Nav construction yields between individual movement samples, rather than
// spending a whole route/edge inside one simulation tick.
fn walk_sample(position: Vec3, target: Vec2, tuning: SimulationTuning) -> Option<Vec3> {
    let world = level::world().character(tuning);
    let step = (target - position.xz()).clamp_length_max(CELL / 6.0);
    let motion = world.move_character(position, Vec3::new(step.x, -0.06, step.y), 1.0, true);
    let next = world.ground(motion.position + Vec3::Y * 0.04, 0.12)?;
    (world.is_grounded(next) && safe_position(next, tuning)).then_some(next)
}

struct BuildingEdge {
    node: usize,
    target: Vec2,
    position: Vec3,
    samples: usize,
}

#[derive(Default)]
struct Graph {
    nodes: Vec<Node>,
    lookup: HashMap<(i32, i32, i32), usize>,
    pending: VecDeque<(usize, usize)>,
    building: Option<BuildingEdge>,
}
struct Node {
    position: Vec3,
    edges: Vec<usize>,
}
impl Graph {
    fn key(position: Vec3) -> (i32, i32, i32) {
        (
            (position.x / CELL).round() as i32,
            (position.z / CELL).round() as i32,
            (position.y * 2.0).round() as i32,
        )
    }
    fn new(sim: SimulationTuning) -> Self {
        let mut graph = Self::default();
        for i in 0..level::data().spawns.len() {
            let spawn = arena::spawn(i, sim).position;
            if safe_position(spawn, sim) && level::world().character(sim).is_grounded(spawn) {
                graph.insert(spawn);
            }
        }
        graph
    }
    fn insert(&mut self, position: Vec3) -> usize {
        let index = self.nodes.len();
        if let Some(&old) = self.lookup.get(&Self::key(position)) {
            return old;
        }
        self.lookup.insert(Self::key(position), index);
        self.nodes.push(Node {
            position,
            edges: Vec::with_capacity(4),
        });
        for direction in 0..4 {
            self.pending.push_back((index, direction));
        }
        index
    }
    fn grow(&mut self, sim: SimulationTuning, budget: Duration) {
        let started = Instant::now();
        for _ in 0..12 {
            if self.building.is_none() {
                let Some((node, direction)) = self.pending.pop_front() else {
                    return;
                };
                let position = self.nodes[node].position;
                let (x, z, _) = Self::key(position);
                let (dx, dz) = [(1, 0), (-1, 0), (0, 1), (0, -1)][direction];
                let target = Vec2::new((x + dx) as f32 * CELL, (z + dz) as f32 * CELL);
                self.building = Some(BuildingEdge {
                    node,
                    target,
                    position,
                    samples: ((target - position.xz()).length() / (CELL / 6.0))
                        .ceil()
                        .clamp(1.0, 16.0) as usize,
                });
            }
            let mut edge = self.building.take().unwrap();
            if let Some(position) = walk_sample(edge.position, edge.target, sim) {
                edge.position = position;
                edge.samples -= 1;
                if edge.samples > 0 {
                    self.building = Some(edge);
                } else if (position.xz() - edge.target).length_squared() < 0.12_f32.powi(2) {
                    let key = Self::key(position);
                    let neighbor =
                        self.lookup.get(&key).copied().or_else(|| {
                            (self.nodes.len() < MAX_NODES).then(|| self.insert(position))
                        });
                    if let Some(neighbor) = neighbor {
                        self.nodes[edge.node].edges.push(neighbor);
                    }
                }
            }
            if started.elapsed() >= budget {
                break;
            }
        }
    }
    fn nearest(&self, position: Vec3) -> Option<usize> {
        self.nodes
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                a.position
                    .distance_squared(position)
                    .total_cmp(&b.position.distance_squared(position))
            })
            .map(|(i, _)| i)
    }
    fn reachable(&self, start: usize) -> Vec<usize> {
        let mut previous = vec![usize::MAX; self.nodes.len()];
        previous[start] = start;
        let mut queue = VecDeque::from([start]);
        for _ in 0..MAX_SEARCH {
            let Some(index) = queue.pop_front() else {
                break;
            };
            for &neighbor in &self.nodes[index].edges {
                if previous[neighbor] == usize::MAX {
                    previous[neighbor] = index;
                    queue.push_back(neighbor);
                }
            }
        }
        previous
    }
    fn route(previous: &[usize], start: usize, mut end: usize) -> VecDeque<usize> {
        let mut route = VecDeque::new();
        while end != start {
            route.push_front(end);
            end = previous[end];
        }
        route
    }
    fn patrol(&self, start: usize, minimum_distance: f32, seed: usize) -> VecDeque<usize> {
        let mut queue = VecDeque::from([start]);
        let mut visited = vec![false; self.nodes.len()];
        visited[start] = true;
        let mut candidates = Vec::new();
        for _ in 0..128 {
            let Some(index) = queue.pop_front() else {
                break;
            };
            if self.nodes[index]
                .position
                .distance_squared(self.nodes[start].position)
                > minimum_distance.powi(2)
            {
                candidates.push(index);
            }
            for &neighbor in &self.nodes[index].edges {
                if !visited[neighbor] {
                    visited[neighbor] = true;
                    queue.push_back(neighbor);
                }
            }
        }
        candidates
            .get(seed % candidates.len().max(1))
            .map_or_else(VecDeque::new, |&goal| self.path(start, goal))
    }
    fn path(&self, start: usize, goal: usize) -> VecDeque<usize> {
        let mut previous = vec![usize::MAX; self.nodes.len()];
        let mut queue = VecDeque::from([start]);
        previous[start] = start;
        let mut best = start;
        for _ in 0..MAX_SEARCH {
            let Some(index) = queue.pop_front() else {
                break;
            };
            if self.nodes[index]
                .position
                .distance_squared(self.nodes[goal].position)
                < self.nodes[best]
                    .position
                    .distance_squared(self.nodes[goal].position)
            {
                best = index;
            }
            if index == goal {
                best = goal;
                break;
            }
            for &neighbor in &self.nodes[index].edges {
                if previous[neighbor] == usize::MAX {
                    previous[neighbor] = index;
                    queue.push_back(neighbor);
                }
            }
        }
        let mut route = VecDeque::new();
        while best != start {
            route.push_front(best);
            best = previous[best];
        }
        route
    }
}
