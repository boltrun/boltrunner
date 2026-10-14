//! Automatic energy pistol. Fire gating rewinds with player prediction;
//! projectile movement and hits are authoritative on the server.
use bevy::{math::Curve, prelude::*};
use parry3d::{
    math::{Pose, Vector},
    query::{ShapeCastOptions, cast_shapes},
    shape::{Ball, Capsule},
};
use serde::{Deserialize, Serialize};

use crate::{PlayerInput, PlayerState, level::CollisionWorld};

pub use crate::arsenal::{WeaponKind, WeaponState, step};

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect)]
pub struct Projectile {
    pub homing_target: Option<u64>,
    pub homing_turn: f32,
    pub homing_range: f32,
    pub homing_cone: f32,
    pub shock_time: f32,
    pub shock_radius: f32,
    pub shock_dps: f32,
    pub flame_reach: f32,
    pub owner: u64,
    pub shot: u16,
    pub pellet: u8,
    pub kind: WeaponKind,
    pub origin: Vec3,
    pub position: Vec3,
    pub direction: Vec3,
    pub remaining_ticks: u16,
    pub lifetime_ticks: u16,
    pub speed: f32,
    pub radius: f32,
    pub damage: f32,
    pub gravity: f32,
    pub bounces: u8,
    pub bounce_loss: f32,
    pub can_hit_owner: bool,
    pub blast_radius: f32,
    pub blast_damage: f32,
    pub knockback: f32,
    pub self_damage: f32,
    pub falloff_start: f32,
    pub falloff_end: f32,
    pub min_damage: f32,
    pub traveled: f32,
    pub relocation: u16,
}

impl Projectile {
    pub fn from_shot(owner: u64, state: &PlayerState, input: &PlayerInput) -> Self {
        let origin = state.position + Vec3::Y * state.tuning.eye_height;
        let kind = state.weapon.selected;
        let stats = kind.stats(state.tuning);
        Self {
            owner,
            homing_target: None,
            homing_turn: if kind == WeaponKind::HomingPlasma {
                state.tuning.seeker_turn
            } else {
                0.0
            },
            homing_range: state.tuning.seeker_range,
            homing_cone: state.tuning.seeker_cone,
            shock_time: state.tuning.electro_field_time,
            shock_radius: state.tuning.electro_field_radius,
            shock_dps: state.tuning.electro_field_dps,
            flame_reach: crate::arsenal::flame_reach(state),
            shot: state.weapon.shot,
            pellet: 0,
            kind,
            origin,
            position: origin,
            direction: Quat::from_euler(EulerRot::YXZ, state.yaw, input.pitch_radians(), 0.0)
                * Vec3::NEG_Z,
            remaining_ticks: crate::tuning::seconds_to_ticks(stats.lifetime).max(1),
            lifetime_ticks: crate::tuning::seconds_to_ticks(stats.lifetime).max(1),
            speed: stats.speed,
            radius: stats.radius,
            damage: stats.damage,
            gravity: stats.gravity,
            bounces: stats.bounces,
            bounce_loss: stats.bounce_loss,
            can_hit_owner: false,
            blast_radius: stats.blast_radius,
            blast_damage: stats.blast_damage,
            knockback: stats.knockback,
            self_damage: stats.self_damage,
            falloff_start: stats.falloff_start,
            falloff_end: stats.falloff_end,
            min_damage: stats.min_damage,
            traveled: 0.0,
            relocation: 0,
        }
    }

    pub fn volley(owner: u64, state: &PlayerState, input: &PlayerInput) -> Vec<Self> {
        let kind = state.weapon.selected;
        let stats = kind.stats(state.tuning);
        let spread = if kind == WeaponKind::MachineGun {
            stats.spread + (state.tuning.mg_max_spread - stats.spread) * state.weapon.heat
        } else {
            stats.spread
        };
        (0..stats.pellets)
            .map(|pellet| {
                let mut bolt = Self::from_shot(owner, state, input);
                bolt.pellet = pellet;
                if spread > 0.0 && kind.is_projectile() {
                    let mut seed = owner
                        ^ ((state.weapon.shot as u64) << 32)
                        ^ ((pellet as u64 + 1) * 0x9e3779b9);
                    let mut random = || {
                        seed ^= seed << 13;
                        seed ^= seed >> 7;
                        seed ^= seed << 17;
                        (seed >> 40) as f32 / 16777216.0
                    };
                    let angle = random() * std::f32::consts::TAU;
                    let radius = random().sqrt() * spread.to_radians().tan();
                    let rotation =
                        Quat::from_euler(EulerRot::YXZ, state.yaw, input.pitch_radians(), 0.0);
                    bolt.direction = (rotation
                        * Vec3::new(angle.cos() * radius, angle.sin() * radius, -1.0))
                    .normalize();
                }
                bolt
            })
            .collect()
    }

    pub fn damage_at(&self, traveled: f32) -> f32 {
        let fade = ((traveled - self.falloff_start)
            / (self.falloff_end - self.falloff_start).max(0.001))
        .clamp(0.0, 1.0);
        self.damage * (1.0 - fade * (1.0 - self.min_damage))
    }

    pub fn guide<'a>(
        &mut self,
        world: &CollisionWorld,
        players: impl IntoIterator<Item = (u64, &'a PlayerState)>,
        dt: f32,
    ) {
        if self.kind != WeaponKind::HomingPlasma || self.homing_turn <= 0.0 {
            return;
        }
        let targets: Vec<_> = players
            .into_iter()
            .filter(|(id, p)| *id != self.owner && p.death.is_none() && !p.match_paused)
            .map(|(id, p)| (id, p.position + Vec3::Y * p.tuning.player_height * 0.5))
            .filter(|(_, target)| {
                let delta = *target - self.position;
                delta.length_squared() <= self.homing_range * self.homing_range
                    && world
                        .sweep_sphere(self.position, delta, 0.005)
                        .is_none_or(|f| f >= 0.995)
            })
            .collect();
        let target = self
            .homing_target
            .and_then(|id| targets.iter().find(|(other, _)| *other == id).copied())
            .or_else(|| {
                targets
                    .iter()
                    .filter(|(_, p)| {
                        (*p - self.position).normalize_or_zero().dot(self.direction)
                            >= self.homing_cone.to_radians().cos()
                    })
                    .min_by(|(a, pa), (b, pb)| {
                        pa.distance_squared(self.position)
                            .total_cmp(&pb.distance_squared(self.position))
                            .then(a.cmp(b))
                    })
                    .copied()
            });
        self.homing_target = target.map(|(id, _)| id);
        if let Some((_, target)) = target {
            let desired = (target - self.position).normalize_or_zero();
            let angle = self.direction.dot(desired).clamp(-1.0, 1.0).acos();
            if angle > 0.00001 {
                let rotation = Quat::from_rotation_arc(self.direction, desired);
                self.direction = (Quat::IDENTITY.slerp(
                    rotation,
                    (self.homing_turn.to_radians() * dt / angle).min(1.0),
                ) * self.direction)
                    .normalize_or_zero();
            }
        }
    }

    pub fn apply_gravity(&mut self, dt: f32) {
        let velocity = self.direction * self.speed - Vec3::Y * self.gravity * dt;
        self.speed = velocity.length();
        self.direction = velocity.normalize_or_zero();
    }
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Reflect)]
pub struct ShockField {
    pub owner: u64,
    pub position: Vec3,
    pub normal: Vec3,
    pub radius: f32,
    pub dps: f32,
    pub remaining_ticks: u16,
    pub total_ticks: u16,
}

/// A capsule's support radius expands the melee sector at its boundary.
pub fn melee_in_sector(
    origin: Vec3,
    direction: Vec3,
    point: Vec3,
    radius: f32,
    range: f32,
    angle: f32,
) -> bool {
    let delta = point - origin;
    let distance = delta.length();
    if distance > range + radius {
        return false;
    }
    if distance <= radius {
        return true;
    }
    let padding = (radius / distance).min(1.0).asin();
    delta.dot(direction) / distance
        >= (angle.to_radians() * 0.5 + padding)
            .min(std::f32::consts::PI)
            .cos()
}

impl Ease for Projectile {
    fn interpolating_curve_unbounded(start: Self, end: Self) -> impl Curve<Self> {
        FunctionCurve::new(Interval::UNIT, move |t: f32| {
            if start.relocation != end.relocation {
                return if t < 1.0 { start.clone() } else { end.clone() };
            }
            Self {
                position: start.position.lerp(end.position, t.clamp(0.0, 1.0)),
                remaining_ticks: if t >= 1.0 {
                    end.remaining_ticks
                } else {
                    start.remaining_ticks
                },
                ..start.clone()
            }
        })
    }
}

#[derive(Debug, PartialEq)]
pub enum Impact {
    Wall,
    Player(u64),
    Drone(u64),
}

pub fn sphere_hit(origin: Vec3, delta: Vec3, center: Vec3, radius: f32) -> Option<f32> {
    let offset = origin - center;
    let c = offset.length_squared() - radius * radius;
    if c <= 0.0 {
        return Some(0.0);
    }
    let a = delta.length_squared();
    if a < 1e-12 {
        return None;
    }
    let b = offset.dot(delta);
    let discriminant = b * b - a * c;
    if discriminant < 0.0 {
        return None;
    }
    let fraction = (-b - discriminant.sqrt()) / a;
    (0.0..=1.0).contains(&fraction).then_some(fraction)
}

/// Sweep the complete segment, choosing the first collision. This prevents a
/// fast bolt tunnelling through thin walls or hitting a player behind a wall.
pub fn trace<'a>(
    world: &CollisionWorld,
    projectile: &Projectile,
    delta: Vec3,
    players: impl IntoIterator<Item = (u64, &'a PlayerState)>,
) -> Option<(f32, Impact)> {
    trace_moving(
        world,
        projectile,
        delta,
        players
            .into_iter()
            .filter(|(_, p)| p.death.is_none())
            .map(|(id, p)| {
                (
                    id,
                    p.position,
                    Vec3::ZERO,
                    p.tuning.player_radius,
                    p.tuning.player_height,
                )
            }),
    )
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ShotImpact {
    pub owner: u64,
    pub shot: u16,
    pub pellet: u8,
    pub kind: WeaponKind,
    pub tick: u32,
    pub position: Vec3,
    pub normal: Vec3,
    pub origin: Vec3,
    pub explosion_radius: f32,
    pub beam_radius: f32,
    pub terminal: bool,
    /// Only populated after the server actually applied damage.
    pub victim: Option<u64>,
}

pub fn trace_moving(
    world: &CollisionWorld,
    projectile: &Projectile,
    delta: Vec3,
    players: impl IntoIterator<Item = (u64, Vec3, Vec3, f32, f32)>,
) -> Option<(f32, Impact)> {
    let mut first = world
        .sweep_sphere(projectile.position, delta, projectile.radius)
        .map(|fraction| (fraction, Impact::Wall));
    for (id, position, movement, radius, height) in players {
        if id == projectile.owner && !projectile.can_hit_owner {
            continue;
        }
        let capsule = Capsule::new_y(height / 2.0 - radius, radius);
        let center = position + Vec3::Y * (height / 2.0);
        let hit = cast_shapes(
            &Pose::translation(center.x, center.y, center.z),
            Vector::new(movement.x, movement.y, movement.z),
            &capsule,
            &Pose::translation(
                projectile.position.x,
                projectile.position.y,
                projectile.position.z,
            ),
            Vector::new(delta.x, delta.y, delta.z),
            &Ball::new(projectile.radius),
            ShapeCastOptions {
                max_time_of_impact: 1.0,
                stop_at_penetration: true,
                ..Default::default()
            },
        )
        .expect("capsule/sphere shape cast is supported");
        if let Some(hit) = hit
            && first
                .as_ref()
                .is_none_or(|(fraction, _)| hit.time_of_impact < *fraction)
        {
            first = Some((hit.time_of_impact, Impact::Player(id)));
        }
    }
    first
}

/// Collision along a potentially warped segment, measured in the original step.
#[derive(Debug)]
pub struct MapImpact {
    pub fraction: f32,
    pub impact: Impact,
    pub normal: Vec3,
}

pub const MAX_WARP_SEGMENTS: usize = 4;

#[derive(Clone, Copy, Debug)]
pub struct RaySegment {
    pub origin: Vec3,
    pub end: Vec3,
}

pub struct RayTrace {
    pub segments: Vec<RaySegment>,
    pub hit: Option<MapImpact>,
}

/// Trace an instantaneous shot through the same apertures as projectiles.
/// Each segment samples targets at the same shot time, not at a travel time
/// proportional to its distance. The returned segments also define the VFX.
pub fn trace_ray_with(
    projectile: &Projectile,
    range: f32,
    mut trace: impl FnMut(&Projectile, Vec3) -> Option<(f32, Impact)>,
) -> RayTrace {
    let mut ray = projectile.clone();
    let mut segments = Vec::with_capacity(MAX_WARP_SEGMENTS);
    let hit = advance_map_with(
        &mut ray,
        projectile.direction * range.max(0.0),
        |ray, delta, _, _| {
            let hit = trace(ray, delta);
            segments.push(RaySegment {
                origin: ray.position,
                end: ray.position + delta * hit.as_ref().map_or(1.0, |(f, _)| *f),
            });
            hit
        },
    );
    RayTrace { segments, hit }
}

/// Split movement at visible portal apertures. The callback sees each segment's
/// start time and duration, allowing moving player and drone hit tests to agree.
pub fn advance_map_with(
    projectile: &mut Projectile,
    mut delta: Vec3,
    mut trace: impl FnMut(&Projectile, Vec3, f32, f32) -> Option<(f32, Impact)>,
) -> Option<MapImpact> {
    let mut elapsed = 0.0;
    let mut remaining = 1.0;
    for _ in 0..MAX_WARP_SEGMENTS {
        let crossing = crate::level::warp_crossing(projectile.position, delta, projectile.radius);
        let fraction = crossing.map_or(1.0, |c| c.fraction);
        let segment = delta * fraction;
        if let Some((hit, impact)) = trace(projectile, segment, elapsed, remaining * fraction) {
            let normal = if impact == Impact::Wall {
                crate::level::world()
                    .sweep_sphere_hit(projectile.position, segment, projectile.radius)
                    .map_or(-projectile.direction, |h| h.normal)
            } else {
                -projectile.direction
            };
            projectile.position += segment * hit;
            projectile.traveled += segment.length() * hit;
            return Some(MapImpact {
                fraction: elapsed + remaining * fraction * hit,
                impact,
                normal,
            });
        }
        projectile.traveled += segment.length();
        let Some(crossing) = crossing else {
            projectile.position += delta;
            return None;
        };
        elapsed += remaining * fraction;
        remaining *= 1.0 - fraction;
        projectile.position = crossing.position;
        projectile.origin = crossing.position;
        projectile.direction = crossing.rotation * projectile.direction;
        projectile.relocation = projectile.relocation.wrapping_add(1);
        delta = crossing.rotation * delta * (1.0 - fraction);
    }
    None
}

pub fn advance_map_hit_moving(
    projectile: &mut Projectile,
    delta: Vec3,
    players: impl IntoIterator<Item = (u64, Vec3, Vec3, f32, f32)>,
) -> Option<MapImpact> {
    let players: Vec<_> = players.into_iter().collect();
    advance_map_with(projectile, delta, |bolt, segment, elapsed, duration| {
        trace_moving(
            crate::level::world(),
            bolt,
            segment,
            players
                .iter()
                .map(|&(id, position, movement, radius, height)| {
                    (
                        id,
                        position + movement * elapsed,
                        movement * duration,
                        radius,
                        height,
                    )
                }),
        )
    })
}

pub fn advance_map_moving(
    projectile: &mut Projectile,
    delta: Vec3,
    players: impl IntoIterator<Item = (u64, Vec3, Vec3, f32, f32)>,
) -> Option<Impact> {
    advance_map_hit_moving(projectile, delta, players).map(|hit| hit.impact)
}
pub fn advance_map(projectile: &mut Projectile, delta: Vec3) -> Option<Impact> {
    advance_map_moving(projectile, delta, std::iter::empty())
}
