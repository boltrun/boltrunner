//! Registered map assets and cached worlds, selected by the authoritative server.
use bevy::prelude::*;
use parry3d::{
    na::{Isometry3, Point3, Vector3},
    query::{ShapeCastOptions, cast_shapes},
    shape::{Ball, Capsule, TriMesh, TriMeshFlags},
};
use serde::Deserialize;
use std::sync::{
    OnceLock,
    atomic::{AtomicUsize, Ordering},
};

use crate::tuning::SimulationTuning;

pub const SKIN: f32 = 0.002;

#[derive(Deserialize)]
pub struct MapData {
    pub id: String,
    pub name: String,
    pub spawns: Vec<Spawn>,
    pub pickup_spawns: Vec<Vec3>,
    pub triggers: Vec<Trigger>,
    pub lightmaps: Vec<String>,
    pub sky: Option<String>,
    pub materials: Vec<MapMaterial>,
}

#[derive(Deserialize, Clone, Copy, Debug)]
pub struct Spawn {
    pub position: Vec3,
    pub yaw: f32,
}

#[derive(Deserialize)]
pub struct MapMaterial {
    pub name: String,
    pub texture: String,
    pub glow: Option<String>,
    pub emissive: bool,
    pub alpha: bool,
    pub blend: bool,
    pub uv_scroll: Vec2,
    pub water: bool,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TriggerKind {
    Push {
        velocity: Vec3,
    },
    Teleport {
        destination: Vec3,
        rotation: f32,
    },
    Warp {
        entrance: Vec3,
        normal: Vec3,
        destination: Vec3,
        exit_normal: Vec3,
        rotation: f32,
    },
    Hurt,
}

#[derive(Deserialize)]
pub struct Trigger {
    #[serde(flatten)]
    pub kind: TriggerKind,
    pub planes: Vec<[f32; 4]>,
}

#[derive(Clone, Copy, Debug)]
pub struct WarpCrossing {
    pub fraction: f32,
    pub position: Vec3,
    pub rotation: Quat,
}

impl Trigger {
    /// Cross the authored visible aperture from its front, never the sides/back.
    pub fn warp_crossing(&self, origin: Vec3, delta: Vec3, radius: f32) -> Option<WarpCrossing> {
        let TriggerKind::Warp {
            entrance,
            normal,
            destination,
            exit_normal,
            rotation,
        } = self.kind
        else {
            return None;
        };
        let start = normal.dot(origin - entrance) - radius;
        let movement = normal.dot(delta);
        if start < -SKIN || movement >= -1e-8 || start + movement > 0.0 {
            return None;
        }
        let fraction = (start / -movement).clamp(0.0, 1.0);
        let point = origin + delta * fraction - normal * radius;
        if self.planes.iter().any(|&[x, y, z, distance]| {
            let plane = Vec3::new(x, y, z);
            plane.dot(normal).abs() < 0.99 && plane.dot(point) > distance - radius + SKIN
        }) {
            return None;
        }
        let rotation = Quat::from_rotation_y(rotation);
        Some(WarpCrossing {
            fraction,
            position: destination + rotation * (point - entrance) + exit_normal * (radius + SKIN),
            rotation,
        })
    }
    pub fn touches(&self, feet: Vec3, tuning: &SimulationTuning) -> bool {
        let center = feet + Vec3::Y * (tuning.player_height / 2.0);
        self.planes.iter().all(|&[x, y, z, distance]| {
            let support = tuning.player_radius
                + (tuning.player_height / 2.0 - tuning.player_radius) * y.abs();
            Vec3::new(x, y, z).dot(center) <= distance + support + SKIN
        })
    }
}

pub struct CollisionWorld {
    mesh: TriMesh,
}

#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub fraction: f32,
    pub normal: Vec3,
}

pub struct Motion {
    pub position: Vec3,
    pub velocity: Vec3,
    pub grounded: bool,
    pub landed: bool,
}

pub struct MapAsset {
    pub id: &'static str,
    metadata: &'static str,
    collision: &'static [u8],
    pub render: &'static [u8],
    data: OnceLock<MapData>,
    world: OnceLock<CollisionWorld>,
    pickups: OnceLock<Vec<Vec3>>,
}

impl MapAsset {
    const fn new(
        id: &'static str,
        metadata: &'static str,
        collision: &'static [u8],
        render: &'static [u8],
    ) -> Self {
        Self {
            id,
            metadata,
            collision,
            render,
            data: OnceLock::new(),
            world: OnceLock::new(),
            pickups: OnceLock::new(),
        }
    }

    pub fn data(&self) -> &MapData {
        self.data.get_or_init(|| {
            let data: MapData =
                serde_json::from_str(self.metadata).expect("compiled map metadata must be valid");
            assert_eq!(data.id, self.id, "map id must match its registry entry");
            assert!(!data.spawns.is_empty(), "map must have player spawns");
            data
        })
    }

    pub fn world(&self) -> &CollisionWorld {
        self.world.get_or_init(|| {
            let mut reader = BinaryReader::new(self.collision);
            let vertices = reader.u32() as usize;
            let triangles = reader.u32() as usize;
            let vertices = (0..vertices)
                .map(|_| Point3::new(reader.f32(), reader.f32(), reader.f32()))
                .collect();
            let indices = (0..triangles)
                .map(|_| [reader.u32(), reader.u32(), reader.u32()])
                .collect();
            reader.finish();
            CollisionWorld::new(vertices, indices)
        })
    }
}

include!(concat!(env!("OUT_DIR"), "/map_assets.rs"));
static ACTIVE: AtomicUsize = AtomicUsize::new(DEFAULT_MAP_INDEX);

pub fn active() -> &'static MapAsset {
    &MAPS[ACTIVE.load(Ordering::Relaxed)]
}

/// Called before joining/starting simulation, never during a simulation tick.
pub fn select(id: &str) -> Result<(), String> {
    let index = MAPS
        .iter()
        .position(|map| map.id == id)
        .ok_or_else(|| format!("Unknown map: {id}"))?;
    let _ = MAPS[index].data();
    let _ = MAPS[index].world();
    ACTIVE.store(index, Ordering::Relaxed);
    Ok(())
}

pub fn data() -> &'static MapData {
    active().data()
}
pub fn world() -> &'static CollisionWorld {
    active().world()
}

/// First visible portal crossed by a moving sphere. Collision must be compared
/// against this fraction before applying the transform.
pub fn warp_crossing(origin: Vec3, delta: Vec3, radius: f32) -> Option<WarpCrossing> {
    data()
        .triggers
        .iter()
        .filter_map(|trigger| trigger.warp_crossing(origin, delta, radius))
        .min_by(|a, b| a.fraction.total_cmp(&b.fraction))
}

/// Authored pickup locations are optional. Empty maps simply have no pickups.
pub fn pickup_slots() -> &'static [Vec3] {
    let map = active();
    map.pickups.get_or_init(|| {
        let mut slots: Vec<Vec3> = Vec::new();
        for &raw in &map.data().pickup_spawns {
            let Some(feet) = map
                .world()
                .character(SimulationTuning::default())
                .ground(raw + Vec3::Y, 3.0)
            else {
                continue;
            };
            if slots
                .iter()
                .all(|other| other.distance_squared(feet) >= 25.0)
            {
                slots.push(feet);
            }
        }
        slots
    })
}

/// Reader for the one current compiler output; no runtime format negotiation.
pub struct BinaryReader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> BinaryReader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    pub fn u32(&mut self) -> u32 {
        let end = self.cursor + 4;
        let value = u32::from_le_bytes(self.bytes[self.cursor..end].try_into().unwrap());
        self.cursor = end;
        value
    }

    pub fn f32(&mut self) -> f32 {
        f32::from_bits(self.u32())
    }

    pub fn finish(self) {
        assert_eq!(
            self.cursor,
            self.bytes.len(),
            "unconsumed compiled map data"
        );
    }
}

impl CollisionWorld {
    /// Keep a third-person camera's near plane in front of level geometry.
    pub fn clip_camera(&self, pivot: Vec3, desired: Vec3, radius: f32) -> Vec3 {
        let delta = desired - pivot;
        if delta.length_squared() < 1e-14 {
            return pivot;
        }
        let fraction = self
            .sweep_sphere(pivot, delta, radius)
            .map_or(1.0, |fraction| {
                (fraction - SKIN / delta.length()).clamp(0.0, 1.0)
            });
        pivot + delta * fraction
    }

    /// Continuous collision for cameras and small, fast energy projectiles.
    pub fn sweep_sphere(&self, origin: Vec3, delta: Vec3, radius: f32) -> Option<f32> {
        cast_shapes(
            &Isometry3::identity(),
            &Vector3::zeros(),
            &self.mesh,
            &Isometry3::translation(origin.x, origin.y, origin.z),
            &Vector3::new(delta.x, delta.y, delta.z),
            &Ball::new(radius),
            ShapeCastOptions {
                max_time_of_impact: 1.0,
                stop_at_penetration: true,
                ..Default::default()
            },
        )
        .expect("triangle mesh/sphere shape cast is supported")
        .map(|hit| hit.time_of_impact)
    }

    pub fn new(vertices: Vec<Point3<f32>>, indices: Vec<[u32; 3]>) -> Self {
        Self {
            mesh: TriMesh::with_flags(
                vertices,
                indices,
                TriMeshFlags::FIX_INTERNAL_EDGES | TriMeshFlags::DELETE_DEGENERATE_TRIANGLES,
            )
            .expect("compiled map must contain valid collision triangles"),
        }
    }

    pub fn character(&self, tuning: SimulationTuning) -> CharacterWorld<'_> {
        CharacterWorld {
            world: self,
            tuning,
            capsule: Capsule::new_y(
                tuning.player_height / 2.0 - tuning.player_radius,
                tuning.player_radius,
            ),
        }
    }
}

pub struct CharacterWorld<'a> {
    world: &'a CollisionWorld,
    tuning: SimulationTuning,
    capsule: Capsule,
}
impl CharacterWorld<'_> {
    pub fn sweep(&self, feet: Vec3, delta: Vec3) -> Option<Hit> {
        if delta.length_squared() < 1e-14 {
            return None;
        }
        let center = feet + Vec3::Y * (self.tuning.player_height / 2.0);
        cast_shapes(
            &Isometry3::identity(),
            &Vector3::zeros(),
            &self.world.mesh,
            &Isometry3::translation(center.x, center.y, center.z),
            &Vector3::new(delta.x, delta.y, delta.z),
            &self.capsule,
            ShapeCastOptions {
                max_time_of_impact: 1.0,
                target_distance: 0.0,
                stop_at_penetration: false,
                compute_impact_geometry_on_penetration: true,
            },
        )
        .expect("triangle mesh/capsule shape cast is supported")
        .map(|hit| {
            let mut normal = Vec3::new(hit.normal1.x, hit.normal1.y, hit.normal1.z);
            // GJK can return a small tangential error at a coplanar triangle seam.
            // Remove that error on axial surfaces so flat floors never create drift.
            for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
                if normal.dot(axis).abs() > 0.9999 {
                    normal = axis * normal.dot(axis).signum();
                    break;
                }
            }
            Hit {
                fraction: (hit.time_of_impact - SKIN / delta.length()).clamp(0.0, 1.0),
                normal,
            }
        })
    }

    pub fn ground(&self, feet: Vec3, distance: f32) -> Option<Vec3> {
        self.sweep(feet, Vec3::NEG_Y * distance)
            .filter(|hit| hit.normal.y >= self.tuning.walkable_normal_y)
            .map(|hit| feet - Vec3::Y * distance * hit.fraction)
    }

    pub fn is_grounded(&self, feet: Vec3) -> bool {
        self.ground(feet, self.tuning.ground_probe_distance)
            .is_some()
    }

    pub fn settle_spawn(&self, spawn: Spawn) -> Spawn {
        Spawn {
            position: self.ground(spawn.position, 4.0).unwrap_or(spawn.position),
            ..spawn
        }
    }

    fn slide(&self, start: Vec3, velocity: Vec3, dt: f32) -> Motion {
        let mut result = Motion {
            position: start,
            velocity,
            grounded: false,
            landed: false,
        };
        let mut remaining = dt;
        let mut planes = Vec::with_capacity(6);
        for _ in 0..6 {
            let delta = result.velocity * remaining;
            let Some(hit) = self.sweep(result.position, delta) else {
                result.position += delta;
                break;
            };
            result.position += delta * hit.fraction;
            if hit.fraction == 0.0 {
                // A capsule exactly touching a wall must be able to slide down it.
                result.position += hit.normal * SKIN;
            }
            remaining *= 1.0 - hit.fraction;
            if hit.normal.y >= self.tuning.walkable_normal_y && result.velocity.y < 0.0 {
                result.grounded = true;
                result.landed = true;
            }
            planes.push(hit.normal);
            // Reclip against earlier contacts to avoid leaking through corners.
            for _ in 0..2 {
                for plane in &planes {
                    result.velocity -= *plane * result.velocity.dot(*plane).min(0.0);
                }
            }
            if result.velocity.length_squared() < 1e-8 || remaining < 1e-6 {
                break;
            }
        }
        result
    }

    pub fn move_character(&self, start: Vec3, velocity: Vec3, dt: f32, grounded: bool) -> Motion {
        let mut motion = self.slide(start, velocity, dt);
        // Step up only from support, with clearance above and a walkable landing.
        if grounded && velocity.y <= 0.0 && velocity.xz().length_squared() > 1e-6 {
            let requested = velocity.xz().length_squared() * dt * dt;
            let achieved = (motion.position - start).xz().length_squared();
            if achieved + 1e-5 < requested
                && self
                    .sweep(start, Vec3::Y * self.tuning.step_height)
                    .is_none()
            {
                let raised = start + Vec3::Y * self.tuning.step_height;
                let mut stepped = self.slide(raised, velocity.with_y(0.0), dt);
                if let Some(floor) = self.ground(
                    stepped.position,
                    self.tuning.step_height + self.tuning.ground_snap_distance,
                ) && (floor - start).xz().length_squared() > achieved + 1e-5
                {
                    stepped.position = floor;
                    stepped.velocity.y = 0.0;
                    stepped.grounded = true;
                    stepped.landed = false;
                    motion = stepped;
                }
            }
        }
        if grounded && velocity.y <= 0.0 {
            let distance = if velocity.xz().length_squared() > 1e-6 {
                self.tuning.step_height + self.tuning.ground_snap_distance
            } else {
                self.tuning.ground_snap_distance
            };
            if let Some(floor) = self.ground(motion.position, distance) {
                motion.position = floor;
                motion.velocity.y = 0.0;
                motion.grounded = true;
            }
        }
        if motion.grounded && motion.velocity.y < 0.0 {
            motion.velocity.y = 0.0;
        }
        motion
    }

    /// Stop an impact boost at a touching wall, including when the floor was hit first.
    pub fn clip_at_wall(&self, feet: Vec3, velocity: Vec3) -> Vec3 {
        if let Some(hit) = self.sweep(feet, velocity.normalize_or_zero() * 0.015)
            && hit.normal.y < self.tuning.walkable_normal_y
        {
            return velocity - hit.normal * velocity.dot(hit.normal).min(0.0);
        }
        velocity
    }
}
