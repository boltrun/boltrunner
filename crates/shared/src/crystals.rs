use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{level::CollisionWorld, tuning::SimulationTuning};

#[derive(Deserialize, Clone, Copy, Debug)]
pub struct CrystalSpawn {
    pub position: Vec3,
    pub radius: f32,
    pub blast_radius: f32,
    pub damage: u16,
    pub respawn_seconds: f32,
}

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Crystal {
    pub slot: u16,
    pub position: Vec3,
    pub active: bool,
    pub remaining_ticks: u16,
}

impl Crystal {
    pub fn new(slot: u16, spawn: &CrystalSpawn) -> Self {
        Self {
            slot,
            position: spawn.position,
            active: true,
            remaining_ticks: 0,
        }
    }

    pub fn detonate(&mut self, spawn: &CrystalSpawn) -> bool {
        if !self.active {
            return false;
        }
        self.active = false;
        self.remaining_ticks = crate::tuning::seconds_to_ticks(spawn.respawn_seconds);
        true
    }

    pub fn advance(&mut self) {
        if !self.active {
            self.remaining_ticks = self.remaining_ticks.saturating_sub(1);
            self.active = self.remaining_ticks == 0;
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CrystalExplosion {
    pub position: Vec3,
    pub radius: f32,
    pub map_revision: u64,
}

/// Continuous bolt/sphere sweep, including an origin already inside the sphere.
pub fn hit_fraction(origin: Vec3, delta: Vec3, position: Vec3, radius: f32) -> Option<f32> {
    let offset = origin - position;
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
    let t = (-b - discriminant.sqrt()) / a;
    (0.0..=1.0).contains(&t).then_some(t)
}

/// The owner and other players use the same distance falloff and wall occlusion.
pub fn blast_damage(
    world: &CollisionWorld,
    spawn: &CrystalSpawn,
    feet: Vec3,
    tuning: &SimulationTuning,
) -> u16 {
    let center = feet + Vec3::Y * (tuning.player_height * 0.5);
    let distance = (center.distance(spawn.position) - tuning.player_radius).max(0.0);
    if distance >= spawn.blast_radius {
        return 0;
    }
    let visible = [0.25, 0.5, 0.85].into_iter().any(|height| {
        let target = feet + Vec3::Y * (tuning.player_height * height);
        world
            .sweep_sphere(spawn.position, target - spawn.position, 0.01)
            .is_none_or(|t| t >= 0.999)
    });
    if !visible {
        return 0;
    }
    (spawn.damage as f32 * (1.0 - distance / spawn.blast_radius)).ceil() as u16
}
