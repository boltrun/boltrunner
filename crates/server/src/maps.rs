use bevy::prelude::*;
use boltrunner_shared::{
    PlayerId, level,
    match_state::MatchState,
    powerups::Pickup,
    protocol::{ChangeMap, LobbyChannel, MapChangeReply},
    tuning::FeatureSettings,
    weapon::Projectile,
};
use lightyear::prelude::{server::ClientOf, *};

pub struct MapsPlugin;
impl Plugin for MapsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PreUpdate,
            change
                .after(MessageSystems::Receive)
                .before(crate::matches::advance),
        );
    }
}

type MapConnections = (
    Entity,
    &'static mut MessageReceiver<ChangeMap>,
    &'static mut MessageSender<MapChangeReply>,
    Has<crate::Joined>,
);
type MapEntities = Or<(
    With<PlayerId>,
    With<Projectile>,
    With<Pickup>,
    With<boltrunner_shared::inventory::GroundWeapon>,
    With<boltrunner_shared::weapon::ShockField>,
)>;

fn change(
    mut commands: Commands,
    mut clients: Query<MapConnections, (With<ClientOf>, With<Connected>)>,
    entities: Query<Entity, MapEntities>,
    mut round: ResMut<MatchState>,
    settings: Res<FeatureSettings>,
    mut clock: ResMut<crate::matches::MatchClock>,
    mut history: ResMut<crate::history::PlayerHistory>,
    mut pickups: ResMut<crate::powerups::PickupDirector>,
) {
    let mut selected = None;
    for (_, mut requests, mut replies, joined) in &mut clients {
        for request in requests.receive() {
            let error = if !joined {
                Some("Join the match before changing maps.".into())
            } else if request.revision != round.map_revision || selected.is_some() {
                Some("The map has already changed. Try again after loading.".into())
            } else if request.map == round.map {
                Some("This map is already active.".into())
            } else if !level::MAPS.iter().any(|map| map.id == request.map) {
                Some("Unknown map.".into())
            } else {
                selected = Some(request.map);
                None
            };
            replies.send::<LobbyChannel>(MapChangeReply { error });
        }
    }
    let Some(map) = selected else { return };
    level::select(&map).expect("validated map");
    for entity in &entities {
        commands.entity(entity).despawn();
    }
    for (entity, _, _, _) in &mut clients {
        commands.entity(entity).remove::<crate::Joined>();
    }
    *history = default();
    *pickups = default();
    clock.restart(settings.values.rules.match_duration);
    *round = MatchState {
        map,
        map_revision: round.map_revision + 1,
        number: round.number + 1,
        remaining_seconds: settings.values.rules.match_duration.ceil() as u32,
        ..default()
    };
    info!(
        "Switched to {} (revision {})",
        round.map, round.map_revision
    );
}
