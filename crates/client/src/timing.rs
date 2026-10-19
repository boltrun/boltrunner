use crate::NetworkStats;
use bevy::prelude::*;
use boltrunner_shared::{PlayerState, TICK_DURATION};
use lightyear::prelude::*;
use std::time::Duration;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct NetworkPresentation;

#[derive(Component)]
pub struct PresentationPosition(pub Vec3);

pub(crate) struct TimingPlugin;
impl Plugin for TimingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CorrectionState>()
            .add_systems(PreUpdate, remember_pose.before(PredictionSystems::Rollback))
            .add_systems(
                PreUpdate,
                capture_correction.after(PredictionSystems::Rollback),
            )
            .add_systems(Update, present_owner.in_set(NetworkPresentation))
            .add_systems(PostUpdate, adapt_buffer.before(SyncSystems::Sync));
    }
}

#[derive(Resource, Default)]
struct CorrectionState {
    before: Option<(Entity, PlayerState)>,
    owner: Option<Entity>,
    relocation: Option<u16>,
    offset: Vec3,
}

fn remember_pose(
    mut correction: ResMut<CorrectionState>,
    players: Query<(Entity, &PlayerState), With<Predicted>>,
) {
    correction.before = players.single().ok().map(|(id, state)| (id, state.clone()));
}

fn capture_correction(
    mut correction: ResMut<CorrectionState>,
    players: Query<(Entity, &PlayerState), With<Predicted>>,
) {
    let Ok((entity, state)) = players.single() else {
        correction.owner = None;
        correction.offset = Vec3::ZERO;
        return;
    };
    if correction.owner != Some(entity) {
        correction.owner = Some(entity);
        correction.relocation = None;
        correction.offset = Vec3::ZERO;
        return;
    }
    let Some((previous_entity, before)) = correction.before.as_ref() else {
        return;
    };
    if *previous_entity != entity
        || before.relocation != state.relocation
        || before.death.is_some() != state.death.is_some()
    {
        correction.offset = Vec3::ZERO;
        return;
    }
    // No ordinary fixed ticks run between these samples: this difference is
    // exclusively reconciliation, not walking/dashing or mouse input.
    let error = before.position - state.position;
    if error.length_squared() > 0.002_f32.powi(2) {
        correction.offset = if error.length() < 2.0 {
            correction.offset + error
        } else {
            Vec3::ZERO
        };
    }
}

fn present_owner(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut correction: ResMut<CorrectionState>,
    mut players: Query<(Entity, &PlayerState, Option<&mut PresentationPosition>), With<Predicted>>,
) {
    correction.offset *= (-25.0 * time.delta_secs()).exp();
    for (entity, state, presentation) in &mut players {
        if correction.relocation != Some(state.relocation) {
            correction.relocation = Some(state.relocation);
            correction.offset = Vec3::ZERO;
        }
        let position = state.position + correction.offset;
        if let Some(mut presentation) = presentation {
            presentation.0 = position;
        } else {
            commands
                .entity(entity)
                .insert(PresentationPosition(position));
        }
    }
}

#[derive(Default)]
struct Arrivals {
    tick: Option<Tick>,
    last_time: f64,
    gap_ms: f32,
    margin_ms: f32,
}

fn adapt_buffer(
    time: Res<Time<Real>>,
    mut arrivals: Local<Arrivals>,
    stats: Res<NetworkStats>,
    links: Query<&PingManager, With<Connected>>,
    mut config: ResMut<InterpolationConfig>,
    mut input_config: ResMut<InputTimelineConfig>,
    updates: Query<&ConfirmedHistory<PlayerState>, With<Interpolated>>,
) {
    let Ok(ping) = links.single() else {
        *arrivals = Arrivals::default();
        return;
    };
    let tick_ms = TICK_DURATION.as_secs_f32() * 1000.0;
    let now = time.elapsed_secs_f64();
    let received = updates
        .iter()
        .filter_map(|history| history.newest_present().map(|(tick, _)| tick))
        .max();
    if received.is_some() && received != arrivals.tick {
        if arrivals.tick.is_some() {
            let gap = ((now - arrivals.last_time) * 1000.0) as f32;
            // Grow quickly for bursts; slowly shed margin after recovery.
            arrivals.gap_ms = if gap > 500.0 {
                0.0
            } else {
                gap.max(arrivals.gap_ms * 0.98)
            };
        }
        arrivals.last_time = now;
        arrivals.tick = received;
    }
    let frame_ms = stats.frame_ms.max(tick_ms).min(50.0);
    // Sending is serviced once per render frame. Predict further ahead at
    // low frame rates so inputs arrive before their authoritative tick; local
    // input delay stays zero.
    *input_config = InputTimelineConfig::default().with_sync_config(SyncConfig {
        jitter_margin: (frame_ms * 1.5).max(tick_ms * 2.0) / tick_ms,
        error_margin: 0.5,
        ..default()
    });
    let delivery_margin = arrivals.gap_ms.min(100.0).max(frame_ms) + tick_ms;
    let desired = ping.rtt().as_secs_f32() * 500.0 + delivery_margin;
    let desired = desired.clamp(tick_ms * 2.0, 250.0);
    arrivals.margin_ms = if arrivals.margin_ms == 0.0 || desired > arrivals.margin_ms {
        desired
    } else {
        arrivals.margin_ms.max(desired)
            - (arrivals.margin_ms - desired).min(time.delta_secs() * 5.0)
    };
    config.min_delay = Duration::from_secs_f32(arrivals.margin_ms / 1000.0);
}
