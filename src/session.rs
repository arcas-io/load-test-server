use crate::error::{Result, ServerError};
use libwebrtc::peerconnection::PeerConnection as LibWebRtcPeerConnection;
use nanoid::nanoid;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use tracing::info;

pub(crate) type SessionStorage = HashMap<String, Session>;
pub(crate) type PeerConnections = HashMap<String, PeerConnection>;

#[derive(Debug, Clone, PartialEq, strum::ToString)]
pub(crate) enum State {
    Created,
    Started,
    Stopped,
}

#[derive(Debug)]
pub(crate) struct PeerConnection {
    id: String,
    session_id: String,
    name: String,
    internal_peer_connection: LibWebRtcPeerConnection,
}

#[derive(Debug)]
pub(crate) struct Session {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) peer_connections: PeerConnections,
    pub(crate) state: State,
    pub(crate) start_time: Option<SystemTime>,
    pub(crate) stop_time: Option<SystemTime>,
}

impl Session {
    pub(crate) fn new(name: String) -> Self {
        let id = nanoid!();
        let peer_connections: PeerConnections = HashMap::new();

        Self {
            id,
            name,
            peer_connections,
            state: State::Created,
            start_time: None,
            stop_time: None,
        }
    }
}

// Add a new session to sessions (in internal state)
pub(crate) fn add_session(name: String, sessions: Arc<Mutex<SessionStorage>>) -> Result<String> {
    let session = Session::new(name);
    let session_id = session.id.clone();

    info!("Added session: {:?}", session);

    sessions.lock()?.insert(session_id.clone(), session);

    Ok(session_id)
}

pub(crate) fn start_session(
    session_id: String,
    sessions: Arc<Mutex<SessionStorage>>,
) -> Result<()> {
    info!("Attempting to start session {}", session_id);

    let mut sessions = sessions.lock()?;
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| ServerError::InvalidSessionError(session_id))?;

    session.state = State::Started;
    session.start_time = Some(SystemTime::now());

    // TODO: implement LibWebRtc here

    info!("Started session: {:?}", session);

    Ok(())
}

pub(crate) fn stop_session(session_id: String, sessions: Arc<Mutex<SessionStorage>>) -> Result<()> {
    info!("Attempting to stop session {}", session_id);

    let mut sessions = sessions.lock()?;
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| ServerError::InvalidSessionError(session_id))?;

    session.state = State::Stopped;
    session.stop_time = Some(SystemTime::now());

    // TODO: implement LibWebRtc here

    info!("Stopped session: {:?}", session);

    Ok(())
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::session::SessionStorage;
    use std::sync::{Arc, Mutex};

    #[test]
    fn it_adds_a_session() {
        let session_storage = SessionStorage::new();
        let sessions = Arc::new(Mutex::new(session_storage));
        let session_id = add_session("New Session".into(), sessions.clone()).unwrap();

        assert_eq!(
            session_id,
            sessions.lock().unwrap().get(&session_id).unwrap().id
        );
    }

    #[test]
    fn it_starts_a_session() {
        let session_storage = SessionStorage::new();
        let sessions = Arc::new(Mutex::new(session_storage));
        let session_id = add_session("New Session".into(), sessions.clone()).unwrap();
        start_session(session_id.clone(), sessions.clone()).unwrap();

        assert_eq!(
            State::Started,
            sessions.lock().unwrap().get(&session_id).unwrap().state
        );
    }

    #[test]
    fn it_stops_a_session() {
        let session_storage = SessionStorage::new();
        let sessions = Arc::new(Mutex::new(session_storage));
        let session_id = add_session("New Session".into(), sessions.clone()).unwrap();
        start_session(session_id.clone(), sessions.clone()).unwrap();
        stop_session(session_id.clone(), sessions.clone()).unwrap();

        assert_eq!(
            State::Stopped,
            sessions.lock().unwrap().get(&session_id).unwrap().state
        );
    }
}
