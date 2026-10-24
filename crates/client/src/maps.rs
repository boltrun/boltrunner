use bevy::prelude::*;
use boltrunner_shared::protocol::{ChangeMap, LobbyChannel, MapChangeReply};
use lightyear::prelude::*;

#[derive(Resource, Default)]
pub struct MapSelection {
    pub pending: bool,
    pub error: Option<String>,
    queued: Option<String>,
    elapsed: f32,
}
impl MapSelection {
    pub fn choose(&mut self, map: &str) {
        if !self.pending {
            self.queued = Some(map.into());
            self.pending = true;
            self.error = None;
            self.elapsed = 0.0;
        }
    }
}
pub struct MapsPlugin;
impl Plugin for MapsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapSelection>()
            .add_systems(Update, sync.after(crate::update_session));
    }
}
fn sync(
    session: Res<crate::Session>,
    time: Res<Time<Real>>,
    mut selection: ResMut<MapSelection>,
    mut links: Query<
        (
            &mut MessageSender<ChangeMap>,
            Option<&mut MessageReceiver<MapChangeReply>>,
        ),
        (With<Client>, With<Connected>),
    >,
) {
    if !session.is_playing() {
        *selection = default();
        return;
    }
    let Ok((mut sender, replies)) = links.single_mut() else {
        return;
    };
    if let Some(mut replies) = replies {
        for reply in replies.receive() {
            if reply.error.is_some() {
                selection.pending = false;
                selection.error = reply.error;
            }
        }
    }
    if let Some(map) = selection.queued.take() {
        sender.send::<LobbyChannel>(ChangeMap {
            map,
            revision: session.map_revision.expect("joined map"),
        });
    }
    if selection.pending {
        selection.elapsed += time.delta_secs();
        if selection.elapsed > 15.0 {
            selection.pending = false;
            selection.error = Some("No response from server. Try again.".into());
        }
    }
}
