//! Model proportions and grip coordinates used by world weapons and pickups.
use bevy::prelude::*;
use boltrunner_shared::arsenal::WeaponKind;

pub struct WeaponGeometry {
    pub scale: Vec3,
    pub grip: Vec3,
    pub support: Option<Vec3>,
    /// The mesh's leftmost point after its authored scene transforms.
    pub min_x: f32,
}

pub fn geometry(kind: WeaponKind) -> WeaponGeometry {
    use WeaponKind::*;
    // Assets contain a 0.75 viewmodel transform. These factors compensate for
    // it in the world: a launcher is about 1.3 m long, a controller 0.23 m wide.
    let (scale, min_x) = match kind {
        Pistol => (Vec3::splat(0.9), -0.0500),
        Flame => (Vec3::new(1.40, 1.15, 1.45), -0.1238),
        Shotgun => (Vec3::new(1.25, 1.05, 1.15), -0.0853),
        MachineGun => (Vec3::new(1.30, 1.15, 1.40), -0.0970),
        Rocket => (Vec3::new(1.50, 1.20, 2.00), -0.0975),
        Laser => (Vec3::new(1.30, 1.10, 1.65), -0.0964),
        Sniper => (Vec3::new(1.10, 1.00, 1.20), -0.0658),
        Drone => (Vec3::ONE, -0.1163),
        Ricochet => (Vec3::new(1.25, 1.05, 1.50), -0.0931),
        Karambit => (Vec3::splat(0.9), -0.0450),
        Arc => (Vec3::new(1.30, 1.10, 1.55), -0.1230),
        Grenade => (Vec3::new(1.25, 1.05, 1.65), -0.0931),
        ElectroPlasma => (Vec3::new(1.25, 1.10, 1.55), -0.1146),
        HomingPlasma => (Vec3::new(1.25, 1.05, 1.60), -0.1093),
    };
    // Coordinates already include the asset's ViewmodelRoot transform. Anchor
    // the scaled model at the palm instead of enlarging it around its receiver.
    let (grip, support) = match kind {
        Pistol => (Vec3::new(0.0, -0.085, 0.125), None),
        Karambit => (Vec3::new(0.0, -0.12, -0.09), None),
        Drone => (
            Vec3::new(0.09, -0.045, -0.12),
            Some(Vec3::new(-0.09, -0.045, -0.12)),
        ),
        _ => (
            Vec3::new(0.0, -0.165, -0.03),
            Some(Vec3::new(-0.065, -0.095, -0.18)),
        ),
    };
    WeaponGeometry {
        scale,
        grip,
        support,
        min_x,
    }
}

pub fn view_scale(kind: WeaponKind) -> Vec3 {
    // The dedicated 70-degree camera needs some framing compensation, while
    // preserving the size difference between a handheld remote and a launcher.
    match kind {
        WeaponKind::Pistol | WeaponKind::Karambit => geometry(kind).scale,
        _ => geometry(kind).scale * 0.75,
    }
}
