use crate::error::Result;
use crate::peer_connection::PeerConnectionQueue;
use crate::session::Session;
use libwebrtc::peerconnection_factory::PeerConnectionFactory;
use log::info;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug)]
pub(crate) struct SharedStateInner {
    pub(crate) data: Data,
    pub(crate) peer_connection_factory: PeerConnectionFactory,
    pub(crate) peer_connection_queue: PeerConnectionQueue,
}

pub(crate) type SharedState = Arc<Mutex<SharedStateInner>>;
pub(crate) type Sessions = HashMap<String, Session>;

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
    pub(crate) fn add_session(&mut self, session: Session) -> Result<()> {
        info!("Adding session: {:?}", session);

        self.sessions.insert(session.id.clone(), session);

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn it_adds_a_session() {
        let session = Session::new("New Session".into());
        let session_id = session.id.clone();
        let mut data = Data::new();
        data.add_session(session).unwrap();
        let added_session = data.sessions.get(&session_id).unwrap();

        assert_eq!(session_id, added_session.id);
    }
}
