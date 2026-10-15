pub mod characters;
pub mod maps;
mod timing;
pub mod tuning;
use bevy::prelude::*;
use boltrunner_shared::{
    PlayerId, PlayerInput, PlayerState, ProtocolPlugin, TICK_DURATION, movement,
    protocol::{JoinRejected, JoinRequest, LobbyChannel, PlayerName},
};
use lightyear::interpolation::timeline::InterpolationConfig;
use lightyear::prelude::{client::*, input::native::*, *};
pub use timing::{NetworkPresentation, PresentationPosition};

#[derive(Resource, Clone)]
pub struct ServerUrl(pub String);

pub struct GameNetworkingPlugin;

#[derive(Resource, Default)]
pub struct NetworkStats {
    pub frame_ms: f32,
    /// Network round-trip time; absent while disconnected.
    pub ping_ms: Option<f32>,
}

impl Plugin for GameNetworkingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ClientPlugins {
            tick_duration: TICK_DURATION,
        })
        .add_plugins(ProtocolPlugin)
        .init_resource::<NetworkStats>()
        .init_resource::<Session>()
        .init_resource::<PredictedShots>()
        .add_plugins(timing::TimingPlugin)
        .add_plugins(tuning::TuningPlugin)
        .add_plugins(maps::MapsPlugin)
        .add_plugins(characters::CharactersPlugin)
        .add_observer(join_game)
        .add_systems(Startup, start_lobby)
        .add_systems(Update, (update_session, send_join, maintain_lobby).chain())
        .add_systems(PreUpdate, attach_input)
        .add_systems(FixedUpdate, predict)
        .add_systems(Update, update_network_stats);
    }
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum SessionPhase {
    #[default]
    Title,
    Connecting,
    Playing,
}

#[derive(Resource, Default)]
pub struct Session {
    pub phase: SessionPhase,
    pub nickname: String,
    pub color: u8,
    pub character: String,
    pub error: Option<String>,
    pub lobby_ready: bool,
    pub map: Option<String>,
    pub map_ready: bool,
    pub map_revision: Option<u64>,
    connection: Option<Entity>,
    join_sent: bool,
    elapsed: f32,
    retry_elapsed: f32,
}

impl Session {
    pub fn is_playing(&self) -> bool {
        self.phase == SessionPhase::Playing
    }
}

#[derive(Event)]
pub struct JoinGame {
    pub nickname: String,
    pub color: u8,
}

fn start_lobby(mut commands: Commands, url: Res<ServerUrl>, mut session: ResMut<Session>) {
    open_connection(&mut commands, &url, &mut session);
}

fn join_game(
    event: On<JoinGame>,
    mut commands: Commands,
    url: Res<ServerUrl>,
    mut session: ResMut<Session>,
) {
    if session.phase != SessionPhase::Title {
        return;
    }
    let nickname = match boltrunner_shared::nickname::validate(&event.nickname) {
        Ok(nickname) => nickname,
        Err(error) => {
            session.error = Some(error.into());
            return;
        }
    };
    if boltrunner_shared::player_color::PlayerColor::new(event.color).is_none() {
        session.error = Some("Choose a valid color.".into());
        return;
    }
    session.nickname = nickname;
    session.color = event.color;
    session.error = None;
    session.elapsed = 0.0;
    session.join_sent = false;
    session.phase = SessionPhase::Connecting;
    if session.connection.is_none() {
        open_connection(&mut commands, &url, &mut session);
    }
}

fn open_connection(commands: &mut Commands, url: &ServerUrl, session: &mut Session) {
    let mut endpoint = url::Url::parse(&url.0).expect("game server URL must be valid");
    endpoint
        .query_pairs_mut()
        .append_pair("build", boltrunner_shared::SIMULATION_BUILD);
    #[cfg(target_arch = "wasm32")]
    let config = ClientConfig;
    #[cfg(not(target_arch = "wasm32"))]
    let config = ClientConfig::builder().with_native_certs().disable_nagle();
    let entity = commands
        .spawn((
            RawClient,
            // Browsers do not expose their local socket address. Ownership is assigned by the server.
            LocalAddr("0.0.0.0:0".parse().unwrap()),
            ReplicationReceiver::default(),
            PredictionManager::default(),
            InterpolationConfig {
                // Updated from measured RTT, jitter, arrival gaps and render cadence.
                send_interval_ratio: 2.0,
                sync: SyncConfig {
                    jitter_margin: std::time::Duration::from_millis(1),
                    error_margin: 0.25,
                    max_error_margin: 30.0,
                    speedup_factor: 1.25,
                    ..default()
                },
                ..default()
            },
            InputTimelineConfig::default().with_sync_config(SyncConfig {
                // Inputs must arrive before the server enters their fixed tick. A sub-tick
                // margin can leave every packet one tick late even on loopback.
                jitter_margin: TICK_DURATION * 2,
                error_margin: 0.5,
                ..default()
            }),
            WebSocketClientIo::from_url(config, endpoint.as_str()),
        ))
        .id();
    commands.entity(entity).insert(Connecting);
    session.connection = Some(entity);
    session.lobby_ready = false;
    session.retry_elapsed = 0.0;
    commands.trigger(Connect { entity });
}

fn send_join(
    mut session: ResMut<Session>,
    mut connections: Query<&mut MessageSender<JoinRequest>, (With<Client>, With<Connected>)>,
) {
    if session.phase != SessionPhase::Connecting || session.join_sent || !session.map_ready {
        return;
    }
    if let Some(mut sender) = session
        .connection
        .and_then(|entity| connections.get_mut(entity).ok())
    {
        sender.send::<LobbyChannel>(JoinRequest {
            map: session.map.clone().expect("loaded server map"),
            map_revision: session.map_revision.expect("server map revision"),
            nickname: session.nickname.clone(),
            color: session.color,
            character: session.character.clone(),
        });
        session.join_sent = true;
    }
}

type SessionReplicas = Or<(
    With<PlayerId>,
    With<boltrunner_shared::weapon::Projectile>,
    With<boltrunner_shared::match_state::MatchState>,
    With<boltrunner_shared::tuning::FeatureSettings>,
    With<boltrunner_shared::powerups::Pickup>,
    With<boltrunner_shared::characters::CharacterCatalog>,
    With<boltrunner_shared::inventory::GroundWeapon>,
    With<boltrunner_shared::weapon::ShockField>,
)>;

pub fn update_session(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut session: ResMut<Session>,
    mut shots: ResMut<PredictedShots>,
    mut connections: Query<
        (Option<&Disconnected>, &mut MessageReceiver<JoinRejected>),
        With<Client>,
    >,
    players: Query<
        (
            &PlayerName,
            &boltrunner_shared::player_color::PlayerColor,
            &boltrunner_shared::characters::PlayerCharacter,
        ),
        With<Predicted>,
    >,
    rounds: Query<&boltrunner_shared::match_state::MatchState>,
    catalogs: Query<Entity, With<boltrunner_shared::characters::CharacterCatalog>>,
    replicas: Query<Entity, SessionReplicas>,
) {
    let Some(entity) = session.connection else {
        return;
    };
    let mut changing = None;
    let mut rejection = None;
    let mut failure = None;
    if let Ok((disconnected, mut replies)) = connections.get_mut(entity) {
        for reply in replies.receive() {
            rejection = Some(reply.reason);
        }
        if disconnected.is_some() {
            failure = Some("Disconnected. Check the server and try again.".into());
        }
    } else {
        failure = Some("Connection lost. Please try again.".into());
    }
    if failure.is_none()
        && let Ok(round) = rounds.single()
        && session.map_revision != Some(round.map_revision)
    {
        if session.map_revision.is_some() {
            // Recreate the connection to discard old replication and prediction timelines.
            changing = Some(session.phase != SessionPhase::Title);
            failure = Some(String::new());
        } else if let Err(error) = boltrunner_shared::level::select(&round.map) {
            failure = Some(error);
        } else {
            session.map = Some(round.map.clone());
            session.map_revision = Some(round.map_revision);
            session.map_ready = false;
        }
    }
    let lobby_ready =
        failure.is_none() && session.map.is_some() && !rounds.is_empty() && !catalogs.is_empty();
    if session.lobby_ready != lobby_ready {
        session.lobby_ready = lobby_ready;
    }
    if let Some(reason) = rejection {
        session.phase = SessionPhase::Title;
        session.join_sent = false;
        session.elapsed = 0.0;
        session.error = Some(reason);
    }
    if session.phase == SessionPhase::Connecting && failure.is_none() {
        if !session.lobby_ready || session.map_ready {
            session.elapsed += time.delta_secs();
        }
        if let Some((name, color, character)) = players.iter().next() {
            session.nickname = name.0.clone();
            session.color = color.0;
            session.character = character.0.clone();
            session.phase = SessionPhase::Playing;
        } else if session.elapsed >= 15.0 && failure.is_none() {
            failure = Some("Connection timed out. Check the server and try again.".into());
        }
    }
    if session.is_playing()
        && let Some((_, _, character)) = players.iter().next()
    {
        session.character = character.0.clone();
    }
    if let Some(error) = failure {
        let show_error = session.phase != SessionPhase::Title;
        commands.trigger(Disconnect { entity });
        commands.entity(entity).try_despawn();
        for replica in &replicas {
            commands.entity(replica).try_despawn();
        }
        shots.0.clear();
        session.connection = None;
        session.map = None;
        session.map_revision = None;
        session.map_ready = false;
        session.phase = SessionPhase::Title;
        session.join_sent = false;
        session.lobby_ready = false;
        if let Some(resume) = changing {
            session.phase = if resume {
                SessionPhase::Connecting
            } else {
                SessionPhase::Title
            };
            session.elapsed = 0.0;
            session.retry_elapsed = 2.0;
            session.error = None;
        } else if show_error {
            session.error = Some(error);
        }
    }
}

fn maintain_lobby(
    mut commands: Commands,
    url: Res<ServerUrl>,
    time: Res<Time<Real>>,
    mut session: ResMut<Session>,
) {
    if session.connection.is_some() || session.is_playing() {
        return;
    }
    session.retry_elapsed += time.delta_secs();
    if session.retry_elapsed >= 2.0 {
        open_connection(&mut commands, &url, &mut session);
    }
}

type LocalPlayerWithoutInput = (
    With<PlayerId>,
    With<Predicted>,
    Without<InputMarker<PlayerInput>>,
);

fn attach_input(mut commands: Commands, players: Query<Entity, LocalPlayerWithoutInput>) {
    for entity in &players {
        commands
            .entity(entity)
            .insert(InputMarker::<PlayerInput>::default());
    }
}

#[derive(Resource, Default)]
pub struct PredictedShots(pub Vec<PredictedShot>);

pub struct PredictedShot {
    pub bolt: boltrunner_shared::weapon::Projectile,
    pub tick: Tick,
}

fn predict(
    session: Res<Session>,
    timeline: Res<LocalTimeline>,
    mut shots: ResMut<PredictedShots>,
    mut players: Query<(&PlayerId, &mut PlayerState, &ActionState<PlayerInput>), With<Predicted>>,
) {
    if !session.map_ready {
        return;
    }
    for (id, mut state, input) in &mut players {
        let previous = state.weapon.shot;
        movement::step(&mut state, &input.0);
        if state.weapon.shot != previous {
            if shots.0.len() >= 256 {
                shots.0.drain(..128);
            }
            let mut shot_state = state.clone();
            shot_state.weapon.selected = state.weapon.last_shot_kind;
            shot_state.weapon.charge_ticks = state.weapon.last_shot_charge;
            for bolt in boltrunner_shared::weapon::Projectile::volley(id.0, &shot_state, &input.0) {
                shots.0.push(PredictedShot {
                    bolt,
                    tick: timeline.tick(),
                });
            }
        }
    }
}

fn update_network_stats(
    time: Res<Time<Real>>,
    connections: Query<&PingManager, With<Connected>>,
    mut stats: ResMut<NetworkStats>,
) {
    let dt = time.delta_secs();
    if dt > 0.0 {
        let frame_ms = dt * 1000.0;
        let weight = 1.0 - (-dt / 0.5).exp();
        stats.frame_ms = if stats.frame_ms == 0.0 {
            frame_ms
        } else {
            stats.frame_ms + (frame_ms - stats.frame_ms) * weight
        };
    }
    stats.ping_ms = connections
        .iter()
        .next()
        .map(|ping| ping.rtt().as_secs_f32() * 1000.0);
}
