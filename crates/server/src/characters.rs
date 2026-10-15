use bevy::prelude::*;
use boltrunner_shared::{characters::*, protocol::LobbyChannel};
use lightyear::prelude::{server::ClientOf, *};

pub struct CharactersPlugin;
impl Plugin for CharactersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CharacterCatalog>()
            .add_systems(Startup, setup)
            .add_systems(
                PreUpdate,
                (reload, change).chain().after(MessageSystems::Receive),
            )
            .add_systems(PostUpdate, publish.before(ReplicationSystems::Send));
    }
}
fn path() -> String {
    std::env::var("BOLTRUNNER_CHARACTERS")
        .unwrap_or_else(|_| "assets/characters/catalog.json".into())
}
fn setup(mut commands: Commands, mut catalog: ResMut<CharacterCatalog>) {
    let json = std::fs::read_to_string(path())
        .unwrap_or_else(|_| include_str!("../../../assets/characters/catalog.json").into());
    *catalog = CharacterCatalog::parse(&json).expect("valid character catalog");
    commands.spawn((catalog.clone(), Replicate::to_clients(NetworkTarget::All)));
}
fn reload(
    time: Res<Time<Real>>,
    mut elapsed: Local<f64>,
    mut last_error: Local<Option<String>>,
    mut catalog: ResMut<CharacterCatalog>,
) {
    if time.elapsed_secs_f64() - *elapsed < 1.0 {
        return;
    }
    *elapsed = time.elapsed_secs_f64();
    let Ok(json) = std::fs::read_to_string(path()) else {
        return;
    };
    match CharacterCatalog::parse(&json) {
        Ok(next) => {
            if *catalog != next {
                *catalog = next;
                info!("Reloaded character catalog");
            }
            *last_error = None;
        }
        Err(error) => {
            if last_error.as_ref() != Some(&error) {
                warn!("Character catalog: {error}");
                *last_error = Some(error);
            }
        }
    }
}
type Connections = (
    Entity,
    &'static mut MessageReceiver<ChangeCharacter>,
    &'static mut MessageSender<CharacterReply>,
    Has<crate::Joined>,
);
fn change(
    catalog: Res<CharacterCatalog>,
    mut connections: Query<Connections, (With<ClientOf>, With<Connected>)>,
    mut players: Query<(&ControlledBy, &mut PlayerCharacter)>,
) {
    for (entity, mut requests, mut replies, joined) in &mut connections {
        for request in requests.receive() {
            let error = if !joined {
                Some("Join the match before changing characters.".into())
            } else if !catalog.accepts(&request.character) {
                Some("This character is no longer available.".into())
            } else if let Some((_, mut character)) =
                players.iter_mut().find(|(owner, _)| owner.owner == entity)
            {
                character.0 = request.character;
                None
            } else {
                Some("Player is unavailable. Try again.".into())
            };
            replies.send::<LobbyChannel>(CharacterReply { error });
        }
    }
}
fn publish(catalog: Res<CharacterCatalog>, mut snapshot: Single<&mut CharacterCatalog>) {
    if **snapshot != *catalog {
        **snapshot = catalog.clone();
    }
}
