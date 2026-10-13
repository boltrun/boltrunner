use bevy::prelude::*;
use boltrunner_shared::{characters::*, protocol::LobbyChannel};
use lightyear::prelude::*;

#[derive(Resource, Default)]
pub struct CharacterSelection {
    pub pending: bool,
    pub error: Option<String>,
    queued: Option<String>,
    elapsed: f32,
}
impl CharacterSelection {
    pub fn choose(&mut self, character: &str) {
        if !self.pending {
            self.queued = Some(character.into());
            self.pending = true;
            self.error = None;
            self.elapsed = 0.0;
        }
    }
}
pub struct CharactersPlugin;
impl Plugin for CharactersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CharacterSelection>()
            .add_systems(Update, sync.after(crate::update_session));
    }
}
fn sync(
    session: Res<crate::Session>,
    time: Res<Time<Real>>,
    mut selection: ResMut<CharacterSelection>,
    mut links: Query<
        (
            &mut MessageSender<ChangeCharacter>,
            &mut MessageReceiver<CharacterReply>,
        ),
        (With<Client>, With<Connected>),
    >,
) {
    if !session.is_playing() {
        *selection = default();
        return;
    }
    let Ok((mut sender, mut replies)) = links.single_mut() else {
        return;
    };
    for reply in replies.receive() {
        selection.pending = false;
        selection.error = reply.error;
    }
    if let Some(character) = selection.queued.take() {
        sender.send::<LobbyChannel>(ChangeCharacter { character });
    }
    if selection.pending {
        selection.elapsed += time.delta_secs();
        if selection.elapsed > 10.0 {
            selection.pending = false;
            selection.error = Some("No response. Try again.".into());
        }
    }
}
