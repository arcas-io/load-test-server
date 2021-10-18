use std::sync::Arc;
use std::time::Duration;

use crate::error::Result;
use crate::peer_connection::PeerConnectionQueue;
use crate::session::Session;
use dashmap::DashMap;
use libwebrtc::peerconnection_factory::PeerConnectionFactory;
use log::info;
use tokio::sync::Mutex;

#[derive(Debug)]
pub(crate) struct SharedState {
    pub(crate) data: Arc<Data>,
    pub(crate) peer_connection_factory: PeerConnectionFactory,
    pub(crate) peer_connection_queue: Arc<Mutex<PeerConnectionQueue>>,
}

pub(crate) type Sessions = DashMap<String, Session>;

/// The in-memory persistent data structure for the server.
///
/// sessions: holds current and past sessions, keyed by session.id
#[derive(Debug)]
pub(crate) struct Data {
    pub(crate) sessions: Sessions,
}

impl Data {
    pub(crate) fn new() -> Self {
        Self {
            sessions: Sessions::new(),
        }
    }

    // Add a new session to sessions (in internal state)
    pub(crate) fn add_session(&self, session: Session) -> Result<()> {
        info!("Adding session: {:?}", session);

        self.sessions.insert(session.id.clone(), session);

        Ok(())
    }
}

impl SharedState {
    pub(crate) fn start_metrics_collection(&self) {
        let data = self.data.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            interval.tick().await;
            loop {
                data.sessions
                    .iter()
                    .for_each(|s| s.value().peer_connection_stats());
                interval.tick().await;
            }
        });
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn it_adds_a_session() {
        let session = Session::new("New Session".into());
        let session_id = session.id.clone();
        let data = Data::new();
        data.add_session(session).unwrap();
        let added_session = data.sessions.get(&session_id).unwrap();

        assert_eq!(session_id, added_session.id);
    }
}
