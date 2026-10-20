//! Authored weapon parts remain attached at rest and move through their own loading paths.
use bevy::prelude::*;
use boltrunner_shared::arsenal::WeaponKind;

fn ramp(p: f32, a: f32, b: f32) -> f32 {
    let t = ((p - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn exchange(p: f32) -> f32 {
    ramp(p, 0.10, 0.34) * (1.0 - ramp(p, 0.58, 0.86))
}
fn stroke(p: f32, a: f32, b: f32) -> f32 {
    if p < a || p > b {
        0.0
    } else {
        (std::f32::consts::PI * (p - a) / (b - a)).sin()
    }
}

pub fn is_part(kind: WeaponKind, name: &str) -> bool {
    use WeaponKind::*;
    match kind {
        Pistol => name == "PistolEnergyCell",
        Flame => name == "FuelTank",
        Shotgun => matches!(name, "ReloadShell" | "MovingPump" | "LoadingGate"),
        MachineGun => matches!(name, "AmmoDrum" | "MovingBolt"),
        Rocket => matches!(name, "ReloadRocket" | "RearCollar"),
        Laser => matches!(name, "ChargeCore" | "EmitterRail"),
        Sniper => matches!(name, "ReloadRifleMagazine" | "MovingBolt"),
        Ricochet => matches!(name, "BallChamber" | "MovingBolt"),
        Arc => name == "Capacitor",
        Grenade => matches!(name, "ReloadPlasmaDrum" | "MovingBolt"),
        ElectroPlasma => name == "ReloadElectroCell",
        HomingPlasma => matches!(name, "ReloadSeekerLeft" | "ReloadSeekerRight"),
        Drone | Karambit => false,
    }
}

pub fn is_round(name: &str) -> bool {
    matches!(name, "ReloadShell" | "ReloadRocket")
}

/// Returns visibility for authored insertion rounds; permanent parts keep normal visibility.
pub fn animate(
    kind: WeaponKind,
    name: &str,
    rest: Transform,
    transform: &mut Transform,
    progress: Option<f32>,
) -> Option<bool> {
    if !is_part(kind, name) {
        return None;
    }
    let Some(p) = progress else {
        return is_round(name).then_some(false);
    };
    let away = exchange(p);
    let unlocked = ramp(p, 0.0, 0.12) * (1.0 - ramp(p, 0.86, 1.0));
    let mut offset = Vec3::ZERO;
    let mut rotation = Quat::IDENTITY;
    let mut pivot = rest.translation;
    use WeaponKind::*;
    match (kind, name) {
        (Pistol, "PistolEnergyCell") => {
            pivot = Vec3::new(0.0, -0.16, 0.15);
            // Clear the entire cell below the grip before exchanging it.
            // Align it beneath the same socket before the upward insertion.
            let drop = ramp(p, 0.08, 0.27) * (1.0 - ramp(p, 0.71, 0.93));
            let side = ramp(p, 0.29, 0.43) * (1.0 - ramp(p, 0.56, 0.69));
            offset = Vec3::new(-0.16 * side, -0.19 * drop, 0.0);
        }
        (Flame, "FuelTank") => {
            // Unscrew the paired canisters in sequence, lift them off their sockets.
            let local = if rest.translation.x < 0.0 {
                p / 0.66
            } else {
                (p - 0.28) / 0.72
            }
            .clamp(0.0, 1.0);
            let a = exchange(local);
            offset = Vec3::new(rest.translation.x.signum() * 0.10 * a, 0.25 * a, -0.08 * a);
            rotation = Quat::from_rotation_y(std::f32::consts::PI * a);
        }
        (MachineGun, "AmmoDrum") => {
            offset = Vec3::new(0.23 * away, 0.04 * away, -0.09 * away);
            rotation = Quat::from_rotation_x(-0.8 * unlocked);
        }
        (Sniper, "ReloadRifleMagazine") => {
            pivot = Vec3::new(0.0, -0.12, -0.01);
            offset = Vec3::new(-0.13 * away, -0.18 * away, -0.07 * away);
            rotation = Quat::from_rotation_x(0.28 * away);
        }
        (Laser, "EmitterRail") => offset.x = rest.translation.x.signum() * 0.055 * unlocked,
        (Laser, "ChargeCore") => {
            offset = Vec3::new(-0.24 * away, 0.13 * away, 0.07 * away);
            rotation = Quat::from_rotation_z(-0.45 * away);
        }
        (Ricochet, "BallChamber") => {
            offset = Vec3::new(-0.18 * away, 0.02 * away, 0.16 * away);
            rotation = Quat::from_rotation_z(1.1 * unlocked);
        }
        (Arc, "Capacitor") => {
            offset = Vec3::new(-0.19 * away, -0.10 * away, 0.06 * away);
            rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2 * unlocked);
        }
        (Grenade, "ReloadPlasmaDrum") => {
            pivot = Vec3::new(0.235, -0.065, 0.055);
            offset = Vec3::new(0.24 * away, 0.04 * away, -0.10 * away);
            rotation = Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2 * away);
        }
        (ElectroPlasma, "ReloadElectroCell") => {
            pivot = Vec3::new(0.13, -0.13, 0.08);
            offset = Vec3::new(0.20 * away, 0.04 * away, -0.10 * away);
            rotation = Quat::from_rotation_z(0.35 * unlocked);
        }
        (HomingPlasma, "ReloadSeekerLeft" | "ReloadSeekerRight") => {
            let side = if name == "ReloadSeekerLeft" {
                -1.0
            } else {
                1.0
            };
            let local = if side < 0.0 {
                p / 0.67
            } else {
                (p - 0.27) / 0.73
            }
            .clamp(0.0, 1.0);
            let a = exchange(local);
            pivot = Vec3::new(side * 0.105, 0.015, -0.25);
            offset = Vec3::new(side * 0.20 * a, 0.08 * a, 0.07 * a);
            rotation = Quat::from_rotation_z(side * 0.45 * a);
        }
        (Shotgun, "MovingPump") => offset.z = 0.055 * stroke(p, 0.80, 1.0),
        (Shotgun, "LoadingGate") => {
            pivot = Vec3::new(0.0, -0.102, -0.175);
            rotation = Quat::from_rotation_x(-0.75 * unlocked);
        }
        (Shotgun, "ReloadShell") => {
            transform.scale = Vec3::ONE;
            let align = ramp(p, 0.10, 0.38);
            let insert = ramp(p, 0.40, 0.86);
            transform.translation = Vec3::new(
                -0.18 * (1.0 - align),
                -0.17 + 0.125 * insert,
                0.04 - 0.10 * align - 0.14 * insert,
            );
            transform.rotation = Quat::from_rotation_x(0.70 * (1.0 - insert));
            return Some(p > 0.06 && p < 0.94);
        }
        (Rocket, "RearCollar") => {
            // Unlock the ring without moving the loading socket off the tube axis.
            rotation = Quat::from_rotation_z(-0.35 * unlocked);
        }
        (Rocket, "ReloadRocket") => {
            transform.scale = Vec3::ONE;
            // The muzzle points along -Z. Start with the nose behind the rear
            // opening (z = 0.385), then slide the entire round down the bore.
            let insertion = ramp(p, 0.20, 0.86);
            transform.translation = Vec3::new(0.0, 0.085, 0.72 - 0.62 * insertion);
            transform.rotation = Quat::IDENTITY;
            return Some(p > 0.10 && p < 0.94);
        }
        (_, "MovingBolt") => offset.z = 0.10 * stroke(p, 0.86, 1.0),
        _ => {}
    }
    transform.translation = pivot + rotation * (rest.translation - pivot) + offset;
    transform.rotation = rotation * rest.rotation;
    None
}

/// Expose the actual loading socket, rather than applying one shared reload dip.
pub fn pose(kind: WeaponKind, p: f32) -> (Vec3, Quat) {
    let a = ramp(p, 0.0, 0.16) * (1.0 - ramp(p, 0.86, 1.0));
    use WeaponKind::*;
    let (shift, angles) = match kind {
        Pistol => (
            Vec3::new(-0.04, 0.08, -0.12),
            Vec3::new(-0.08, -0.15, -0.90),
        ),
        Flame => (
            Vec3::new(-0.09, -0.015, -0.12),
            Vec3::new(-0.10, -0.25, 0.40),
        ),
        Shotgun => (
            Vec3::new(-0.035, 0.035, -0.07),
            Vec3::new(0.22, -0.15, -0.72),
        ),
        MachineGun => (Vec3::new(-0.10, 0.035, -0.11), Vec3::new(-0.18, 0.15, 0.68)),
        Rocket => (Vec3::new(-0.09, 0.08, -0.45), Vec3::new(0.15, -0.50, -0.48)),
        Laser => (
            Vec3::new(-0.06, 0.015, -0.10),
            Vec3::new(-0.06, 0.28, -0.22),
        ),
        Sniper => (Vec3::new(-0.03, 0.06, -0.09), Vec3::new(0.12, -0.12, -0.58)),
        Ricochet => (Vec3::new(-0.05, 0.04, -0.08), Vec3::new(-0.12, 0.12, -0.30)),
        Arc => (Vec3::new(-0.04, 0.055, -0.08), Vec3::new(0.12, 0.08, -0.62)),
        Grenade => (Vec3::new(-0.10, 0.02, -0.12), Vec3::new(-0.12, 0.16, 0.64)),
        ElectroPlasma => (Vec3::new(-0.09, 0.04, -0.12), Vec3::new(-0.08, 0.20, 0.58)),
        HomingPlasma => (Vec3::new(-0.07, 0.02, -0.13), Vec3::new(-0.10, 0.25, 0.20)),
        Drone | Karambit => (Vec3::ZERO, Vec3::ZERO),
    };
    (
        shift * a,
        Quat::from_euler(EulerRot::YXZ, angles.y * a, angles.x * a, angles.z * a),
    )
}
