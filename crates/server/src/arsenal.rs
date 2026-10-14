use bevy::prelude::*;
use boltrunner_shared::{
    PlayerId, PlayerInput, PlayerState, TICK_DURATION,
    arsenal::WeaponKind,
    health, level,
    match_state::MatchState,
    tuning::{FeatureSettings, MatchTuning, SimulationTuning, seconds_to_ticks},
    weapon::{self, Impact, Projectile, ShotImpact},
};
use lightyear::prelude::{input::native::ActionState, *};

#[derive(Component)]
pub struct ShotRequest {
    pub bolt: Projectile,
    pub tuning: SimulationTuning,
    pub lag: f32,
}
type Players<'w, 's> = Query<
    'w,
    's,
    (
        &'static PlayerId,
        &'static mut PlayerState,
        &'static ActionState<PlayerInput>,
    ),
>;

fn damage(
    state: &mut PlayerState,
    victim: u64,
    owner: u64,
    amount: f32,
    pitch: f32,
    round: &mut MatchState,
    rules: MatchTuning,
) {
    if state.death.is_some() || state.match_paused || amount <= 0.0 {
        return;
    }
    state.damage_fraction += amount;
    let whole = state.damage_fraction.floor().min(u16::MAX as f32) as u16;
    state.damage_fraction -= whole as f32;
    if health::apply_damage(state, whole, pitch) {
        round.record_death(victim, Some(owner), rules);
    }
}

fn drone_damage(state: &mut PlayerState, amount: f32) {
    if let Some(drone) = &mut state.drone {
        drone.damage_fraction += amount;
        let whole = drone.damage_fraction.floor().min(u16::MAX as f32) as u16;
        drone.damage_fraction -= whole as f32;
        drone.health = drone.health.saturating_sub(whole);
        if drone.health == 0 {
            drone.detonate = true;
        }
    }
}

fn clear_line(origin: Vec3, target: Vec3) -> bool {
    level::world()
        .sweep_sphere(origin, target - origin, 0.005)
        .is_none_or(|fraction| fraction >= 0.995)
}

fn trace(
    bolt: &Projectile,
    delta: Vec3,
    players: &Players,
    history: &crate::history::PlayerHistory,
    tick: Tick,
    lag: f32,
) -> Option<(f32, Impact)> {
    trace_segment(bolt, delta, players, history, tick, lag, 0.0, 1.0)
}
fn trace_segment(
    bolt: &Projectile,
    delta: Vec3,
    players: &Players,
    history: &crate::history::PlayerHistory,
    tick: Tick,
    lag: f32,
    elapsed: f32,
    duration: f32,
) -> Option<(f32, Impact)> {
    let mut hit = weapon::trace_moving(
        level::world(),
        bolt,
        delta,
        players.iter().filter_map(|(id, state, _)| {
            if state.match_paused {
                return None;
            }
            let (position, movement) = history
                .sweep(id.0, tick, lag, state)
                .unwrap_or((state.position, Vec3::ZERO));
            state.death.is_none().then_some((
                id.0,
                position + movement * elapsed,
                movement * duration,
                state.tuning.player_radius,
                state.tuning.player_height,
            ))
        }),
    );
    for (id, state, _) in players.iter() {
        if id.0 == bolt.owner && !bolt.can_hit_owner {
            continue;
        }
        if let Some(drone) = state.drone.filter(|d| !d.detonate) {
            if let Some(fraction) = weapon::sphere_hit(
                bolt.position,
                delta,
                drone.position,
                state.tuning.drone_radius + bolt.radius,
            ) {
                if hit.as_ref().is_none_or(|(first, _)| fraction < *first) {
                    hit = Some((fraction, Impact::Drone(id.0)));
                }
            }
        }
    }
    hit
}

fn direct_hit(
    impact: &Impact,
    bolt: &Projectile,
    amount: f32,
    players: &mut Players,
    round: &mut MatchState,
    rules: MatchTuning,
) -> Option<u64> {
    match *impact {
        Impact::Player(victim) => {
            for (id, mut state, input) in players.iter_mut() {
                if id.0 == victim && state.death.is_none() && !state.match_paused {
                    damage(
                        &mut state,
                        victim,
                        bolt.owner,
                        amount,
                        input.0.pitch_radians(),
                        round,
                        rules,
                    );
                    return Some(victim);
                }
            }
        }
        Impact::Drone(owner) => {
            for (id, mut state, _) in players.iter_mut() {
                if id.0 == owner {
                    drone_damage(&mut state, amount);
                    break;
                }
            }
        }
        Impact::Wall => {}
    }
    None
}

fn explosion(
    center: Vec3,
    owner: u64,
    radius: f32,
    amount: f32,
    impulse: f32,
    self_damage: f32,
    inner: bool,
    owner_protected: bool,
    players: &mut Players,
    round: &mut MatchState,
    rules: MatchTuning,
) -> Option<u64> {
    let mut damaged = None;
    for (id, mut state, input) in players.iter_mut() {
        let target = state.position + Vec3::Y * (state.tuning.player_height / 2.0);
        let distance = (target.distance(center) - state.tuning.player_radius).max(0.0);
        if state.death.is_none()
            && !state.match_paused
            && !(id.0 == owner && owner_protected)
            && distance < radius
            && clear_line(center, target)
        {
            let fraction = if inner {
                (2.0 - 2.0 * distance / radius).clamp(0.0, 1.0)
            } else {
                (1.0 - distance / radius).clamp(0.0, 1.0)
            };
            let direction = (target - center).normalize_or_zero();
            state.velocity += direction.xz() * impulse * fraction;
            state.vertical_velocity += direction.y * impulse * fraction;
            damage(
                &mut state,
                id.0,
                owner,
                amount * fraction * if id.0 == owner { self_damage } else { 1.0 },
                input.0.pitch_radians(),
                round,
                rules,
            );
            if id.0 != owner {
                damaged = Some(id.0);
            }
        }
        if let Some(drone) = state.drone {
            let distance = drone.position.distance(center);
            if distance < radius && clear_line(center, drone.position) {
                drone_damage(&mut state, amount * (1.0 - distance / radius));
            }
        }
    }
    damaged
}

fn event(
    bolt: &Projectile,
    tick: Tick,
    position: Vec3,
    normal: Vec3,
    victim: Option<u64>,
    explosion_radius: f32,
    terminal: bool,
) -> ShotImpact {
    ShotImpact {
        owner: bolt.owner,
        shot: bolt.shot,
        pellet: bolt.pellet,
        kind: bolt.kind,
        tick: tick.0,
        position,
        normal,
        origin: bolt.origin,
        victim,
        explosion_radius,
        beam_radius: bolt.radius,
        terminal,
    }
}
fn send(event: ShotImpact, clients: &mut Query<&mut MessageSender<ShotImpact>, With<Connected>>) {
    for mut sender in clients.iter_mut() {
        sender.send::<boltrunner_shared::protocol::LobbyChannel>(event.clone());
    }
}

pub fn resolve_shots(
    mut commands: Commands,
    shots: Query<(Entity, &ShotRequest)>,
    timeline: Res<LocalTimeline>,
    history: Res<crate::history::PlayerHistory>,
    mut clients: Query<&mut MessageSender<ShotImpact>, With<Connected>>,
    mut players: Players,
    mut round: ResMut<MatchState>,
    settings: Res<FeatureSettings>,
) {
    for (entity, shot) in &shots {
        commands.entity(entity).despawn();
        if round.results {
            continue;
        }
        let bolt = &shot.bolt;
        let stats = bolt.kind.stats(shot.tuning);
        let mut victim = None;
        if bolt.kind == WeaponKind::Karambit {
            let mut contact = bolt.clone();
            for (id, mut state, input) in &mut players {
                if id.0 == bolt.owner || state.death.is_some() || state.match_paused {
                    continue;
                }
                let (feet, _) = history
                    .sweep(id.0, timeline.tick(), shot.lag, &state)
                    .unwrap_or((state.position, Vec3::ZERO));
                let half = state.tuning.player_height * 0.5 - state.tuning.player_radius;
                let center = feet + Vec3::Y * state.tuning.player_height * 0.5;
                let touched = [center - Vec3::Y * half, center, center + Vec3::Y * half]
                    .into_iter()
                    .find(|point| {
                        weapon::melee_in_sector(
                            bolt.origin,
                            bolt.direction,
                            *point,
                            state.tuning.player_radius,
                            stats.range,
                            shot.tuning.karambit_angle,
                        ) && clear_line(bolt.origin, *point)
                    });
                if let Some(point) = touched {
                    damage(
                        &mut state,
                        id.0,
                        bolt.owner,
                        stats.damage,
                        input.0.pitch_radians(),
                        &mut round,
                        settings.values.rules,
                    );
                    send(
                        event(
                            &contact,
                            timeline.tick(),
                            point,
                            -bolt.direction,
                            Some(id.0),
                            0.0,
                            true,
                        ),
                        &mut clients,
                    );
                    contact.pellet = contact.pellet.wrapping_add(1);
                }
                if let Some(drone) = state.drone.filter(|d| !d.detonate) {
                    if weapon::melee_in_sector(
                        bolt.origin,
                        bolt.direction,
                        drone.position,
                        state.tuning.drone_radius,
                        stats.range,
                        shot.tuning.karambit_angle,
                    ) && clear_line(bolt.origin, drone.position)
                    {
                        drone_damage(&mut state, stats.damage);
                        send(
                            event(
                                &contact,
                                timeline.tick(),
                                drone.position,
                                -bolt.direction,
                                None,
                                0.0,
                                true,
                            ),
                            &mut clients,
                        );
                    }
                }
            }
        } else if bolt.kind == WeaponKind::Flame {
            let reach = bolt.flame_reach;
            let reach = level::world()
                .sweep_sphere(bolt.origin, bolt.direction * reach, 0.01)
                .map_or(reach, |fraction| reach * fraction);
            for (id, mut state, input) in &mut players {
                if id.0 == bolt.owner || state.death.is_some() || state.match_paused {
                    continue;
                }
                let target = state.position + Vec3::Y * state.tuning.player_height * 0.5;
                let half_segment = state.tuning.player_height / 2.0 - state.tuning.player_radius;
                let reachable = [
                    target - Vec3::Y * half_segment,
                    target,
                    target + Vec3::Y * half_segment,
                ]
                .into_iter()
                .any(|point| {
                    let line = point - bolt.origin;
                    let along = line.dot(bolt.direction);
                    along >= 0.0
                        && along <= reach + state.tuning.player_radius
                        && (line - bolt.direction * along).length()
                            <= along * stats.spread.to_radians().tan() + state.tuning.player_radius
                        && clear_line(bolt.origin, point)
                });
                if reachable {
                    damage(
                        &mut state,
                        id.0,
                        bolt.owner,
                        stats.damage,
                        input.0.pitch_radians(),
                        &mut round,
                        settings.values.rules,
                    );
                    if state.death.is_none() {
                        state.burn_ticks = seconds_to_ticks(shot.tuning.flame_burn_time);
                        state.burn_dps = shot.tuning.flame_burn_dps;
                        state.burn_owner = Some(bolt.owner);
                    }
                    victim = Some(id.0);
                }
                if let Some(drone) = state.drone {
                    let line = drone.position - bolt.origin;
                    let along = line.dot(bolt.direction);
                    if along >= 0.0
                        && along <= reach
                        && (line - bolt.direction * along).length()
                            <= along * stats.spread.to_radians().tan() + state.tuning.drone_radius
                        && clear_line(bolt.origin, drone.position)
                    {
                        drone_damage(&mut state, stats.damage);
                    }
                }
            }
            // Stream geometry is rendered continuously from replicated weapon state.
            if victim.is_some() {
                send(
                    event(
                        bolt,
                        timeline.tick(),
                        bolt.origin + bolt.direction * reach,
                        -bolt.direction,
                        victim,
                        0.0,
                        true,
                    ),
                    &mut clients,
                );
            }
        } else {
            let ray = weapon::trace_ray_with(bolt, stats.range, |segment, delta| {
                trace(
                    segment,
                    delta,
                    &players,
                    &history,
                    timeline.tick(),
                    shot.lag,
                )
            });
            if let Some(hit) = &ray.hit {
                victim = direct_hit(
                    &hit.impact,
                    bolt,
                    stats.damage,
                    &mut players,
                    &mut round,
                    settings.values.rules,
                );
            }
            for (index, segment) in ray.segments.iter().enumerate() {
                let terminal = index + 1 == ray.segments.len();
                let direction = (segment.end - segment.origin).normalize_or_zero();
                let mut effect = bolt.clone();
                effect.origin = segment.origin;
                effect.pellet = index as u8;
                send(
                    event(
                        &effect,
                        timeline.tick(),
                        segment.end,
                        if terminal {
                            ray.hit.as_ref().map_or(-direction, |hit| hit.normal)
                        } else {
                            -direction
                        },
                        if terminal { victim } else { None },
                        0.0,
                        terminal,
                    ),
                    &mut clients,
                );
            }
            if bolt.kind == WeaponKind::Arc {
                // Chain only after a real first target. Each jump chooses the
                // nearest visible living opponent, never revisiting a player.
                let first = ray.hit.as_ref().and_then(|hit| match &hit.impact {
                    Impact::Player(id) => Some(*id),
                    _ => None,
                });
                if let Some(first) = first {
                    let mut visited = vec![bolt.owner, first];
                    let mut origin = ray.segments.last().expect("traced arc segment").end;
                    let mut amount = stats.damage;
                    for jump in 1..=shot.tuning.arc_chains as u8 {
                        let target = players
                            .iter()
                            .filter(|(id, state, _)| {
                                !visited.contains(&id.0)
                                    && state.death.is_none()
                                    && !state.match_paused
                            })
                            .map(|(id, state, _)| {
                                (
                                    id.0,
                                    state.position + Vec3::Y * state.tuning.player_height * 0.5,
                                )
                            })
                            .filter(|(_, point)| {
                                point.distance(origin) <= shot.tuning.arc_chain_range
                                    && clear_line(origin, *point)
                            })
                            .min_by(|(id_a, a), (id_b, b)| {
                                a.distance_squared(origin)
                                    .total_cmp(&b.distance_squared(origin))
                                    .then(id_a.cmp(id_b))
                            });
                        let Some((id, point)) = target else { break };
                        amount *= shot.tuning.arc_retention;
                        visited.push(id);
                        let victim = direct_hit(
                            &Impact::Player(id),
                            bolt,
                            amount,
                            &mut players,
                            &mut round,
                            settings.values.rules,
                        );
                        let mut segment = bolt.clone();
                        segment.origin = origin;
                        segment.pellet = ray.segments.len() as u8 + jump - 1;
                        send(
                            event(
                                &segment,
                                timeline.tick(),
                                point,
                                (origin - point).normalize_or_zero(),
                                victim,
                                0.0,
                                true,
                            ),
                            &mut clients,
                        );
                        origin = point;
                    }
                }
            }
        }
    }
}

pub fn advance_projectiles(
    mut commands: Commands,
    mut projectiles: Query<(Entity, &mut Projectile, &crate::history::ProjectileLag)>,
    timeline: Res<LocalTimeline>,
    history: Res<crate::history::PlayerHistory>,
    mut clients: Query<&mut MessageSender<ShotImpact>, With<Connected>>,
    mut round: ResMut<MatchState>,
    settings: Res<FeatureSettings>,
    mut players: Players,
) {
    let dt = TICK_DURATION.as_secs_f32();
    for (entity, mut bolt, lag) in &mut projectiles {
        if round.results {
            commands.entity(entity).despawn();
            continue;
        }
        bolt.guide(
            level::world(),
            players.iter().map(|(id, state, _)| (id.0, state)),
            dt,
        );
        bolt.apply_gravity(dt);
        let mut remaining = dt;
        let mut terminal = false;
        // Sweep the remainder after every reflection, so no travel is lost at a bounce.
        for _ in 0..4 {
            let delta = bolt.direction * bolt.speed * remaining;
            let prefix = (dt - remaining) / dt;
            let span = remaining / dt;
            let hit =
                weapon::advance_map_with(&mut bolt, delta, |bolt, segment, elapsed, duration| {
                    trace_segment(
                        bolt,
                        segment,
                        &players,
                        &history,
                        timeline.tick(),
                        lag.0,
                        prefix + elapsed * span,
                        duration * span,
                    )
                });
            if let Some(weapon::MapImpact {
                fraction,
                impact,
                normal,
            }) = hit
            {
                if impact == Impact::Wall && bolt.kind.rebounds() && bolt.bounces > 0 {
                    bolt.bounces -= 1;
                    bolt.can_hit_owner = true;
                    bolt.direction = (bolt.direction - 2.0 * bolt.direction.dot(normal) * normal)
                        .normalize_or_zero();
                    bolt.speed *= bolt.bounce_loss;
                    bolt.position += normal * 0.005;
                    send(
                        event(
                            &bolt,
                            timeline.tick(),
                            bolt.position,
                            normal,
                            None,
                            0.0,
                            false,
                        ),
                        &mut clients,
                    );
                    remaining *= 1.0 - fraction;
                    if remaining < 1e-6 {
                        break;
                    }
                    continue;
                }
                let mut victim = direct_hit(
                    &impact,
                    &bolt,
                    bolt.damage_at(bolt.traveled),
                    &mut players,
                    &mut round,
                    settings.values.rules,
                );
                let position = bolt.position + normal * 0.015;
                if bolt.kind == WeaponKind::ElectroPlasma
                    && matches!(impact, Impact::Wall)
                    && bolt.shock_time > 0.0
                {
                    let total = seconds_to_ticks(bolt.shock_time).max(1);
                    commands.spawn((
                        weapon::ShockField {
                            owner: bolt.owner,
                            position: bolt.position - normal * bolt.radius + normal * 0.02,
                            normal,
                            radius: bolt.shock_radius,
                            dps: bolt.shock_dps,
                            remaining_ticks: total,
                            total_ticks: total,
                        },
                        Replicate::to_clients(NetworkTarget::All),
                    ));
                }
                if bolt.blast_radius > 0.0 {
                    let splash = explosion(
                        position,
                        bolt.owner,
                        bolt.blast_radius,
                        bolt.blast_damage,
                        bolt.knockback,
                        bolt.self_damage,
                        false,
                        false,
                        &mut players,
                        &mut round,
                        settings.values.rules,
                    );
                    victim = victim.or(splash);
                }
                send(
                    event(
                        &bolt,
                        timeline.tick(),
                        position,
                        normal,
                        victim,
                        bolt.blast_radius,
                        true,
                    ),
                    &mut clients,
                );
                terminal = true;
                break;
            } else {
                break;
            }
        }
        bolt.remaining_ticks = bolt.remaining_ticks.saturating_sub(1);
        if !terminal && bolt.remaining_ticks == 0 {
            if bolt.blast_radius > 0.0 {
                let victim = explosion(
                    bolt.position,
                    bolt.owner,
                    bolt.blast_radius,
                    bolt.blast_damage,
                    bolt.knockback,
                    bolt.self_damage,
                    false,
                    false,
                    &mut players,
                    &mut round,
                    settings.values.rules,
                );
                send(
                    event(
                        &bolt,
                        timeline.tick(),
                        bolt.position,
                        Vec3::Y,
                        victim,
                        bolt.blast_radius,
                        true,
                    ),
                    &mut clients,
                );
            }
            terminal = true;
        }
        if terminal {
            commands.entity(entity).despawn();
        }
    }
}

pub fn advance_effects(
    mut players: Players,
    mut round: ResMut<MatchState>,
    settings: Res<FeatureSettings>,
    timeline: Res<LocalTimeline>,
    history: Res<crate::history::PlayerHistory>,
    mut clients: Query<&mut MessageSender<ShotImpact>, With<Connected>>,
) {
    if round.results {
        return;
    }
    let mut explosions = Vec::new();
    let drones: Vec<_> = players
        .iter()
        .filter_map(|(id, state, _)| {
            state
                .drone
                .map(|drone| (id.0, drone, state.tuning.drone_radius))
        })
        .collect();
    let mut drone_contacts = Vec::new();
    for (i, (owner, a, radius_a)) in drones.iter().enumerate() {
        for (other, b, radius_b) in drones.iter().skip(i + 1) {
            let delta_a = a.position - a.previous_position;
            let delta_b = b.position - b.previous_position;
            if let Some(fraction) = weapon::sphere_hit(
                a.previous_position,
                delta_a - delta_b,
                b.previous_position,
                radius_a + radius_b,
            ) && clear_line(
                a.previous_position + delta_a * fraction,
                b.previous_position + delta_b * fraction,
            ) {
                drone_contacts.push((*owner, fraction));
                drone_contacts.push((*other, fraction));
            }
        }
    }
    let bodies: Vec<_> = players
        .iter()
        .filter_map(|(id, state, _)| {
            if state.death.is_some() || state.match_paused {
                return None;
            }
            let (position, movement) = history
                .sweep(id.0, timeline.tick(), 0.0, state)
                .unwrap_or((state.position, Vec3::ZERO));
            Some((
                id.0,
                position,
                movement,
                state.tuning.player_radius,
                state.tuning.player_height,
            ))
        })
        .collect();
    for (id, mut state, input) in &mut players {
        if let Some(drone) = state.drone {
            let delta = drone.position - drone.previous_position;
            let mut probe = Projectile::from_shot(id.0, &state, &input.0);
            probe.position = drone.previous_position;
            probe.radius = state.tuning.drone_radius;
            probe.can_hit_owner = drone.owner_grace_ticks == 0;
            // A zero-velocity shape cast does not report an existing overlap.
            // Check contact at the start as well, including a hovering drone.
            let touching = bodies.iter().any(|(other, feet, _, radius, height)| {
                if *other == id.0 && !probe.can_hit_owner {
                    return false;
                }
                let axis = Vec3::new(
                    feet.x,
                    drone
                        .previous_position
                        .y
                        .clamp(feet.y + radius, feet.y + height - radius),
                    feet.z,
                );
                drone.previous_position.distance_squared(axis) <= (radius + probe.radius).powi(2)
                    && clear_line(drone.previous_position, axis)
            });
            let body_contact = if touching {
                Some(0.0)
            } else {
                weapon::trace_moving(level::world(), &probe, delta, bodies.iter().copied())
                    .and_then(|(fraction, impact)| {
                        matches!(impact, Impact::Player(_)).then_some(fraction)
                    })
            };
            let contact = drone_contacts
                .iter()
                .filter(|(owner, _)| *owner == id.0)
                .map(|(_, fraction)| *fraction)
                .chain(body_contact)
                .min_by(f32::total_cmp);
            if let Some(fraction) = contact {
                let drone = state.drone.as_mut().expect("active drone");
                drone.position = drone.previous_position + delta * fraction;
                drone.detonate = true;
            }
        }
        if state.burn_ticks > 0 {
            state.burn_ticks -= 1;
            let amount = state.burn_dps * TICK_DURATION.as_secs_f32();
            let owner = state.burn_owner.unwrap_or(id.0);
            damage(
                &mut state,
                id.0,
                owner,
                amount,
                input.0.pitch_radians(),
                &mut round,
                settings.values.rules,
            );
        }
    }
    collect_detonations(&mut players, &mut explosions);
    let mut next = 0;
    while next < explosions.len() {
        let (owner, position, t, shot, owner_protected) = explosions[next];
        next += 1;
        let victim = explosion(
            position,
            owner,
            t.drone_blast_radius,
            t.drone_damage as f32,
            t.drone_knockback,
            t.drone_self_damage,
            true,
            owner_protected,
            &mut players,
            &mut round,
            settings.values.rules,
        );
        send(
            ShotImpact {
                owner,
                shot,
                pellet: 0,
                kind: WeaponKind::Drone,
                tick: timeline.tick().0,
                position,
                normal: Vec3::Y,
                origin: position,
                victim,
                explosion_radius: t.drone_blast_radius,
                beam_radius: 0.0,
                terminal: true,
            },
            &mut clients,
        );
        // A blast can kill an owner or destroy another drone. Resolve the
        // resulting detonations now, before a zero-delay respawn clears state.
        collect_detonations(&mut players, &mut explosions);
    }
}

fn collect_detonations(
    players: &mut Players,
    explosions: &mut Vec<(u64, Vec3, SimulationTuning, u16, bool)>,
) {
    for (id, mut state, _) in players.iter_mut() {
        if let Some(drone) = state.drone.filter(|d| d.detonate || state.death.is_some()) {
            let tuning = state.tuning;
            explosions.push((
                id.0,
                drone.position,
                tuning,
                state.weapon.shot,
                drone.owner_grace_ticks > 0,
            ));
            state.drone = None;
            state.weapon.cooldowns[WeaponKind::Drone.index()] =
                seconds_to_ticks(tuning.drone_cooldown);
        }
    }
}

pub fn publish_equipment(
    mut players: Query<(
        &PlayerState,
        &mut boltrunner_shared::equipment::PlayerEquipment,
    )>,
) {
    for (state, mut equipment) in &mut players {
        let selected = state.weapon.selected.index() as u8;
        if equipment.weapon != selected {
            equipment.weapon = selected;
        }
    }
}

pub fn advance_fields(
    mut commands: Commands,
    mut fields: Query<(Entity, &mut weapon::ShockField)>,
    mut players: Players,
    mut round: ResMut<MatchState>,
    settings: Res<FeatureSettings>,
) {
    for (entity, mut field) in &mut fields {
        if round.results {
            commands.entity(entity).despawn();
            continue;
        }
        for (id, mut state, input) in &mut players {
            let center = state.position + Vec3::Y * state.tuning.player_height * 0.5;
            let axis = (center
                - Vec3::Y * (state.tuning.player_height * 0.5 - state.tuning.player_radius))
                .lerp(
                    center
                        + Vec3::Y * (state.tuning.player_height * 0.5 - state.tuning.player_radius),
                    ((field.position.y - (state.position.y + state.tuning.player_radius))
                        / (state.tuning.player_height - 2.0 * state.tuning.player_radius)
                            .max(0.001))
                    .clamp(0.0, 1.0),
                );
            if field.position.distance(axis) <= field.radius + state.tuning.player_radius
                && clear_line(field.position, center)
            {
                damage(
                    &mut state,
                    id.0,
                    field.owner,
                    field.dps * TICK_DURATION.as_secs_f32(),
                    input.0.pitch_radians(),
                    &mut round,
                    settings.values.rules,
                );
            }
        }
        field.remaining_ticks = field.remaining_ticks.saturating_sub(1);
        if field.remaining_ticks == 0 {
            commands.entity(entity).despawn();
        }
    }
}
