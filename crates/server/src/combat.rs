use bevy::prelude::*;
use boltrunner_shared::weapon::Projectile;
use lightyear::prelude::*;

/// Publish only bolts that survived this frame’s collision steps. An immediate
/// wall hit must not enqueue a network despawn for a never-published entity.
pub fn publish_projectiles(
    mut commands: Commands,
    bolts: Query<Entity, (With<Projectile>, Without<Replicate>)>,
) {
    for entity in &bolts {
        commands.entity(entity).insert((
            Replicate::to_clients(NetworkTarget::All),
            InterpolationTarget::to_clients(NetworkTarget::All),
            NetworkVisibility,
        ));
    }
}

/// A peer becomes a projectile recipient only when its spawn can be buffered.
/// Otherwise a short-lived bolt can disappear between that peer's send ticks,
/// and Lightyear would send a despawn without ever sending the spawn.
pub fn expose_projectiles(
    mut bolts: Query<&mut ReplicationState, With<Projectile>>,
    senders: Query<(Entity, &ReplicationSender), With<Connected>>,
) {
    for (entity, sender) in &senders {
        if sender.send_timer.is_finished() {
            for mut state in &mut bolts {
                state.gain_visibility(entity);
            }
        }
    }
}
