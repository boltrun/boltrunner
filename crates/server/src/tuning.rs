use bevy::prelude::*;
use boltrunner_shared::{
    protocol::LobbyChannel,
    tuning::{EditReply, EditRequest, FeatureSettings},
};
use lightyear::prelude::{server::ClientOf, *};

#[derive(Resource, Default, Deref, DerefMut)]
pub struct ServerSettings(pub FeatureSettings);

pub struct TuningPlugin;
impl Plugin for TuningPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ServerSettings>()
            .add_systems(Startup, setup)
            .add_systems(PreUpdate, edit.after(MessageSystems::Receive))
            .add_systems(PostUpdate, publish.before(ReplicationSystems::Send));
    }
}
fn setup(mut commands: Commands, settings: Res<ServerSettings>) {
    commands.spawn((
        settings.0.clone(),
        Replicate::to_clients(NetworkTarget::All),
    ));
}
type EditingClients = (With<ClientOf>, With<Connected>);
type EditConnection = (
    &'static mut MessageReceiver<EditRequest>,
    &'static mut MessageSender<EditReply>,
    Has<crate::Joined>,
);
pub(crate) fn edit(
    mut settings: ResMut<ServerSettings>,
    mut clients: Query<EditConnection, EditingClients>,
) {
    for (mut requests, mut replies, joined) in &mut clients {
        for request in requests.receive() {
            let error = if joined {
                settings.apply(request.revision, &request.edit).err()
            } else {
                Some("Join the match before editing settings.".into())
            };
            replies.send::<LobbyChannel>(EditReply {
                id: request.id,
                settings: settings.0.clone(),
                error,
            });
        }
    }
}

fn publish(settings: Res<ServerSettings>, mut snapshot: Single<&mut FeatureSettings>) {
    if **snapshot != settings.0 {
        **snapshot = settings.0.clone();
    }
}
