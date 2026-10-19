mod build_check;
mod combat;
mod history;
mod loading;
mod maps;
mod matches;
mod powerups;
mod tuning;

use bevy::{app::ScheduleRunnerPlugin, prelude::*};
use boltrunner_shared::{
    PlayerId, PlayerInput, PlayerState, ProtocolPlugin, SEND_INTERVAL, TICK_DURATION, TICK_HZ,
    arena, movement,
    player_color::{PALETTE, PlayerColor},
    protocol::{JoinRejected, JoinRequest, LobbyChannel, PlayerName},
    weapon::Projectile,
};
use lightyear::prelude::{input::native::ActionState, server::*, *};
use std::{net::SocketAddr, time::Duration};

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--print-build") {
        println!("{}", boltrunner_shared::SIMULATION_BUILD);
        return;
    }
    let map = std::env::var("BOLTRUNNER_MAP")
        .unwrap_or_else(|_| boltrunner_shared::level::active().id.into());
    if let Err(error) = boltrunner_shared::level::select(&map) {
        eprintln!(
            "{error}. Available maps: {}",
            boltrunner_shared::level::MAPS
                .iter()
                .map(|m| m.id)
                .collect::<Vec<_>>()
                .join(", ")
        );
        std::process::exit(2);
    }
    let address: SocketAddr = std::env::var("BOLTRUNNER_BIND")
        .unwrap_or_else(|_| "0.0.0.0:5000".into())
        .parse()
        .expect("BOLTRUNNER_BIND must be an IP:port");
    App::new()
        .add_plugins((
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_millis(2))),
            bevy::state::app::StatesPlugin,
            bevy::log::LogPlugin::default(),
        ))
        .add_plugins(ServerPlugins {
            tick_duration: TICK_DURATION,
        })
        .add_plugins(ProtocolPlugin)
        .insert_resource(ReplicationMetadata::new(SEND_INTERVAL))
        .add_plugins(tuning::TuningPlugin)
        .add_plugins(loading::LoadingPlugin)
        .add_plugins(maps::MapsPlugin)
        .insert_resource(BindAddress(address))
        .init_resource::<matches::CurrentMatch>()
        .init_resource::<matches::MatchClock>()
        .init_resource::<history::PlayerHistory>()
        .init_resource::<powerups::PickupDirector>()
        .add_systems(Startup, (start, matches::setup))
        .add_systems(PreUpdate, matches::advance.after(tuning::edit))
        .add_observer(configure_link)
        .add_systems(Update, spawn_players)
        .add_observer(log_disconnect)
        .add_systems(
            FixedUpdate,
            (
                history::capture_before,
                simulate,
                history::capture_after,
                powerups::collect,
                powerups::manage,
                combat::advance_projectiles,
            )
                .chain(),
        )
        .add_systems(
            PostUpdate,
            (combat::publish_projectiles, matches::publish).before(ReplicationSystems::Send),
        )
        .run();
}

#[derive(Resource)]
struct BindAddress(SocketAddr);

fn start(mut commands: Commands, address: Res<BindAddress>) {
    loading::prepare_world(&mut commands);
    let config = lightyear::websocket::server::ServerConfig::builder()
        .with_bind_address(address.0)
        .with_no_encryption()
        .with_handshake_handler(build_check::handshake());
    let entity = commands
        .spawn((
            RawServer,
            LocalAddr(address.0),
            WebSocketServerIo { config },
        ))
        .id();
    info!(
        "Loading game [3/4, 75%]: opening WebSocket listener at {}",
        address.0
    );
    commands.trigger(Start { entity });
    info!(
        "Boltrunner: ws://{} ({TICK_HZ:.0} Hz simulation, {TICK_HZ:.0} Hz snapshots)",
        address.0
    );
    info!("Simulation build: {}", boltrunner_shared::SIMULATION_BUILD);
}

fn configure_link(event: On<Add, LinkOf>, mut commands: Commands) {
    commands.entity(event.entity).insert(ReplicationSender);
}

#[derive(Component)]
struct Joined;

fn spawn_players(
    mut links: Query<
        (
            Entity,
            &RemoteId,
            &mut MessageReceiver<JoinRequest>,
            &mut MessageSender<JoinRejected>,
            Has<Joined>,
        ),
        (With<ClientOf>, With<Connected>),
    >,
    players: Query<(&PlayerState, &PlayerColor)>,
    round: Res<crate::matches::CurrentMatch>,
    settings: Res<crate::tuning::ServerSettings>,
    mut commands: Commands,
) {
    let mut occupied = Vec::new();
    let mut occupied_colors = [false; PALETTE.len()];
    for (state, color) in &players {
        occupied.push(state.position);
        occupied_colors[color.0 as usize] = true;
    }
    for (entity, remote, mut requests, mut replies, already_joined) in &mut links {
        let mut joined = already_joined;
        for request in requests.receive() {
            if joined {
                continue;
            }
            if request.map != round.map || request.map_revision != round.map_revision {
                replies.send::<LobbyChannel>(JoinRejected {
                    reason: "The server map changed. Reconnect before joining.".into(),
                });
                continue;
            }
            let name = match boltrunner_shared::nickname::validate(&request.nickname) {
                Ok(name) => name,
                Err(reason) => {
                    replies.send::<LobbyChannel>(JoinRejected {
                        reason: reason.into(),
                    });
                    continue;
                }
            };
            let Some(color) = PlayerColor::new(request.color) else {
                replies.send::<LobbyChannel>(JoinRejected {
                    reason: "Choose a valid color.".into(),
                });
                continue;
            };
            if occupied_colors[color.0 as usize] {
                replies.send::<LobbyChannel>(JoinRejected {
                    reason: "This color is taken. Choose another.".into(),
                });
                continue;
            }
            if occupied.len() >= arena::MAX_PLAYERS {
                replies.send::<LobbyChannel>(JoinRejected {
                    reason: "The server is full. Please try again later.".into(),
                });
                continue;
            }
            let spawn_index = arena::choose_spawn(
                rand::random::<u32>() as usize,
                &occupied,
                settings.values.simulation.spawn_separation,
            );
            let spawn = arena::spawn(spawn_index, settings.values.simulation);
            let position = spawn.position;
            occupied.push(position);
            occupied_colors[color.0 as usize] = true;
            let id = rand::random::<u64>();
            commands.spawn((
                PlayerId(id),
                PlayerName(name.clone()),
                color,
                PlayerState {
                    tuning: settings.values.simulation,
                    health: boltrunner_shared::health::Health(
                        settings.values.simulation.max_health as u16,
                    ),
                    jump: boltrunner_shared::protocol::JumpState {
                        air_jumps_remaining: settings.values.simulation.air_jumps as u8,
                        ..default()
                    },
                    dash: boltrunner_shared::protocol::DashState::new(settings.values.simulation),
                    position,
                    yaw: spawn.yaw,
                    view_yaw_offset: spawn.yaw,
                    spawn_index: spawn_index as u8,
                    grounded: true,
                    match_paused: round.results,
                    ..default()
                },
                Replicate::to_clients(NetworkTarget::All),
                PredictionTarget::to_clients(NetworkTarget::Single(remote.0)),
                InterpolationTarget::to_clients(NetworkTarget::AllExceptSingle(remote.0)),
                ControlledBy {
                    owner: entity,
                    lifetime: Default::default(),
                },
            ));
            commands.entity(entity).insert(Joined);
            joined = true;
            info!(
                "Player {id:016x} ({name}) joined at {position} ({:?})",
                remote.0
            );
        }
    }
}

fn simulate(
    mut commands: Commands,
    mut round: ResMut<crate::matches::CurrentMatch>,
    settings: Res<crate::tuning::ServerSettings>,
    mut players: Query<(
        &PlayerId,
        &mut PlayerState,
        &ActionState<PlayerInput>,
        &ControlledBy,
    )>,
    links: Query<(
        &PingManager,
        Option<&lightyear::interpolation::plugin::InterpolationDelay>,
    )>,
) {
    for (id, mut state, input, control) in &mut players {
        // The settings snapshot rewinds with PlayerState on predicted clients.
        state.tuning = settings.values.simulation;
        let was_alive = state.death.is_none();
        let previous_shot = state.weapon.shot;
        movement::step(&mut state, &input.0);
        if was_alive && state.death.is_some() {
            round.record_death(id.0, None, settings.values.rules);
        }
        if state.weapon.shot != previous_shot {
            let lag = links
                .get(control.owner)
                .ok()
                .map_or(0.0, |(ping, delay)| history::validated_delay(ping, delay));
            commands.spawn((
                Projectile::from_shot(id.0, &state, &input.0),
                history::ProjectileLag(lag),
            ));
        }
    }
}

fn log_disconnect(event: On<Add, Disconnected>, links: Query<&RemoteId, With<ClientOf>>) {
    if let Ok(remote) = links.get(event.entity) {
        info!("Player disconnected: {:?}", remote.0);
    }
}
