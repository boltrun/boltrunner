use bevy::prelude::*;
use boltrunner_shared::{
    PlayerId, PlayerInput, PlayerState, TICK_DURATION,
    crystals::{self, Crystal, CrystalExplosion},
    health, level,
    weapon::{self, Impact, Projectile, ShotImpact},
};
use lightyear::prelude::{input::native::ActionState, *};

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
        ));
    }
}

pub fn advance_projectiles(
    mut commands: Commands,
    mut projectiles: Query<(Entity, &mut Projectile, &crate::history::ProjectileLag)>,
    timeline: Res<LocalTimeline>,
    history: Res<crate::history::PlayerHistory>,
    mut clients: Query<
        (
            &mut MessageSender<ShotImpact>,
            &mut MessageSender<CrystalExplosion>,
        ),
        With<Connected>,
    >,
    mut crystals: Query<&mut Crystal>,
    mut round: ResMut<crate::matches::CurrentMatch>,
    settings: Res<crate::tuning::ServerSettings>,
    mut players: Query<(&PlayerId, &mut PlayerState, &ActionState<PlayerInput>)>,
) {
    if round.results {
        return;
    }
    for (entity, mut bolt, lag) in &mut projectiles {
        let delta = bolt.direction * bolt.speed * TICK_DURATION.as_secs_f32();
        let hit = weapon::advance_map_with_crystals(
            &mut bolt,
            delta,
            players.iter().filter_map(|(id, state, _)| {
                history
                    .sweep(id.0, timeline.tick(), lag.0, state)
                    .map(|(position, movement)| {
                        (
                            id.0,
                            position,
                            movement,
                            state.tuning.player_radius,
                            state.tuning.player_height,
                        )
                    })
            }),
            crystals.iter().filter(|c| c.active).map(|c| {
                (
                    c.slot,
                    c.position,
                    level::data().crystals[c.slot as usize].radius,
                )
            }),
        );
        if let Some(impact) = hit {
            let mut damaged = None;
            if let Impact::Crystal(slot) = impact {
                let spawn = &level::data().crystals[slot as usize];
                if let Some(mut crystal) = crystals.iter_mut().find(|c| c.slot == slot)
                    && crystal.detonate(spawn)
                {
                    for (id, mut state, input) in &mut players {
                        if state.death.is_some() || state.match_paused {
                            continue;
                        }
                        let damage = crystals::blast_damage(
                            level::world(),
                            spawn,
                            state.position,
                            &state.tuning,
                        );
                        if damage > 0
                            && health::apply_damage(&mut state, damage, input.0.pitch_radians())
                        {
                            round.record_death(id.0, Some(bolt.owner), settings.values.rules);
                        }
                    }
                    let explosion = CrystalExplosion {
                        position: spawn.position,
                        radius: spawn.blast_radius,
                        map_revision: round.map_revision,
                    };
                    for (_, mut sender) in &mut clients {
                        sender.send::<boltrunner_shared::protocol::LobbyChannel>(explosion.clone());
                    }
                }
            }
            if let Impact::Player(victim) = impact {
                for (id, mut state, input) in &mut players {
                    if id.0 == victim && state.death.is_none() && !state.match_paused {
                        damaged = Some(victim);
                        if health::apply_damage(&mut state, bolt.damage, input.0.pitch_radians()) {
                            round.record_death(victim, Some(bolt.owner), settings.values.rules);
                        }
                        break;
                    }
                }
            }
            let event = ShotImpact {
                owner: bolt.owner,
                shot: bolt.shot,
                tick: timeline.tick().0,
                position: bolt.position,
                normal: -bolt.direction,
                victim: damaged,
            };
            for (mut sender, _) in &mut clients {
                sender.send::<boltrunner_shared::protocol::LobbyChannel>(event.clone());
            }
            commands.entity(entity).despawn();
            continue;
        }
        bolt.remaining_ticks -= 1;
        if bolt.remaining_ticks == 0 {
            commands.entity(entity).despawn();
        }
    }
}
