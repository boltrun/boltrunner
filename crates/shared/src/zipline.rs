use crate::{
    PlayerInput, PlayerState, TICK_DURATION,
    level::{CollisionWorld, Trigger, TriggerKind},
};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Press history and the ride rewind together with ordinary movement.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Reflect, Default)]
pub struct ZiplineState {
    pub ride: Option<Ride>,
    pub last_press: u16,
    pub reattach_ticks: u16,
}
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Reflect)]
pub struct Ride {
    pub trigger: u16,
    pub progress: f32,
    pub reverse: bool,
}

#[derive(Clone, Copy)]
struct Boarding {
    trigger: usize,
    progress: f32,
    reverse: bool,
    position: Vec3,
}

/// The nearest reachable point along a cable, shared by the hint and prediction.
pub fn nearby(world: &CollisionWorld, triggers: &[Trigger], state: &PlayerState) -> Option<usize> {
    boarding(world, triggers, state, state.yaw).map(|candidate| candidate.trigger)
}

fn boarding(
    world: &CollisionWorld,
    triggers: &[Trigger],
    state: &PlayerState,
    yaw: f32,
) -> Option<Boarding> {
    if state.death.is_some()
        || state.match_paused
        || state.zipline.ride.is_some()
        || state.zipline.reattach_ticks > 0
        || state.tuning.zipline_speed <= 0.0
        || state.drone.is_some_and(|d| d.controlling)
    {
        return None;
    }
    let character = world.character(state.tuning);
    let forward = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
    triggers
        .iter()
        .enumerate()
        .filter_map(|(index, trigger)| {
            let TriggerKind::Zipline { start, end } = trigger.kind else {
                return None;
            };
            let delta = end - start;
            if delta.length_squared() < 0.01 {
                return None;
            }
            let along =
                ((state.position - start).dot(delta) / delta.length_squared()).clamp(0.0, 1.0);
            let position = start + delta * along;
            let distance = state.position.distance_squared(position);
            if distance > state.tuning.zipline_reach.powi(2) {
                return None;
            }
            // Capture uses the real capsule, including at midair points and across walls.
            let dt = TICK_DURATION.as_secs_f32();
            let capture = character.move_character(
                state.position,
                (position - state.position) / dt,
                dt,
                false,
            );
            if capture.position.distance_squared(position) >= 0.0004 {
                return None;
            }
            let reverse = if along <= 0.0001 {
                false
            } else if along >= 0.9999 {
                true
            } else {
                forward.dot(delta) < 0.0
            };
            Some((
                Boarding {
                    trigger: index,
                    progress: if reverse { 1.0 - along } else { along },
                    reverse,
                    position,
                },
                distance,
            ))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(candidate, _)| candidate)
}

/// Returns true when this tick's body movement was handled by the cable.
/// The caller still applies hazards, pickups and ordinary weapon simulation.
pub fn step_in_world(
    world: &CollisionWorld,
    triggers: &[Trigger],
    state: &mut PlayerState,
    input: &PlayerInput,
) -> bool {
    let pressed = input.interact_press != state.zipline.last_press;
    state.zipline.last_press = input.interact_press;
    state.zipline.reattach_ticks = state.zipline.reattach_ticks.saturating_sub(1);
    if state.zipline.ride.is_none()
        && pressed
        && let Some(candidate) = boarding(
            world,
            triggers,
            state,
            input.yaw_radians() + state.view_yaw_offset,
        )
    {
        state.position = candidate.position;
        state.zipline.ride = Some(Ride {
            trigger: candidate.trigger as u16,
            progress: candidate.progress,
            reverse: candidate.reverse,
        });
        state.dash.remaining_ticks = 0;
        state.jump.air_jumps_remaining = state.tuning.air_jumps as u8;
    }
    let Some(ride) = state.zipline.ride else {
        return false;
    };
    let Some(Trigger {
        kind: TriggerKind::Zipline { start, end },
        ..
    }) = triggers.get(ride.trigger as usize)
    else {
        detach(state);
        return false;
    };
    let (start, end) = if ride.reverse {
        (*end, *start)
    } else {
        (*start, *end)
    };
    if start.distance_squared(end) < 0.01 {
        detach(state);
        return false;
    }
    let direction = (end - start).normalize_or_zero();
    let velocity = direction * state.tuning.zipline_speed;
    state.velocity = velocity.xz();
    state.vertical_velocity = velocity.y;
    let jump = input.jump_press != state.jump.last_press;
    let dash = input.dash_press != state.dash.last_press
        && state.dash.charges > 0
        && state.tuning.dash_time > 0.0;
    if jump || dash || state.tuning.zipline_speed <= 0.0 {
        detach(state);
        if jump {
            state.jump.last_press = input.jump_press;
            state.vertical_velocity += state.tuning.jump_speed;
        }
        // Normal movement applies the dash, gravity and momentum this tick.
        return false;
    }
    let dt = TICK_DURATION.as_secs_f32();
    let progress = (ride.progress + state.tuning.zipline_speed * dt / start.distance(end)).min(1.0);
    let desired = start.lerp(end, progress);
    let character = world.character(state.tuning);
    let motion =
        character.move_character(state.position, (desired - state.position) / dt, dt, false);
    state.position = motion.position;
    state.grounded = false;
    state.yaw = input.yaw_radians() + state.view_yaw_offset;
    state.speed_ticks = state.speed_ticks.saturating_sub(1);
    state.rapid_fire_ticks = state.rapid_fire_ticks.saturating_sub(1);
    crate::movement::recharge_dash(state);
    state.jump.last_press = input.jump_press;
    state.dash.last_press = input.dash_press;
    if motion.position.distance_squared(desired) > 0.0004 {
        // Moving along a cable still uses the real player capsule and walls.
        state.velocity = motion.velocity.xz();
        state.vertical_velocity = motion.velocity.y;
        detach(state);
    } else if progress >= 1.0 {
        detach(state);
        // Arrive on the platform without launching the rider over its edge.
        state.velocity = Vec2::ZERO;
        state.vertical_velocity = 0.0;
        state.grounded = character.is_grounded(state.position);
    } else {
        state.zipline.ride = Some(Ride { progress, ..ride });
    }
    true
}

fn detach(state: &mut PlayerState) {
    state.zipline.ride = None;
    state.zipline.reattach_ticks = crate::tuning::seconds_to_ticks(0.25);
    state.grounded = false;
}
