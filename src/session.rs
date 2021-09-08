use crate::error::{Result, ServerError};
use crate::stats::{get_stats, Stats};
use libwebrtc::peerconnection::PeerConnection as LibWebRtcPeerConnection;
use nanoid::nanoid;
use std::collections::HashMap;
use std::time::SystemTime;
use tracing::info;

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

    pub(crate) fn start(&mut self) -> Result<()> {
        info!("Attempting to start session {}", self.id);

        if self.state != State::Created {
            return Err(ServerError::InvalidStateError(
                "Only a created session can be started".into(),
            ));
        }

        self.state = State::Started;
        self.start_time = Some(SystemTime::now());

        // TODO: implement LibWebRtc here

        info!("Started session: {:?}", self);

        Ok(())
    }

    pub(crate) fn stop(&mut self) -> Result<()> {
        info!("Attempting to stop session {}", self.id);

        if self.state != State::Started {
            return Err(ServerError::InvalidStateError(
                "Only a started session can be stopped".into(),
            ));
        }

        self.state = State::Stopped;
        self.stop_time = Some(SystemTime::now());

        // TODO: implement LibWebRtc here

        info!("stopped session: {:?}", self);

        Ok(())
    }

    pub(crate) fn get_stats(&self) -> Result<Stats> {
        info!("Attempting to get stats for session {}", self.id);

        let stats = get_stats(&self)?;

        info!("Stats for session {}: {:?}", self.id, stats);

        Ok(stats)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::data::Data;

    #[test]
    fn it_adds_a_session() {
        let session = Session::new("New Session".into());
        let session_id = session.id.clone();
        let mut data = Data::new();
        data.add_session(session).unwrap();

        assert_eq!(session_id, data.sessions.get(&session_id).unwrap().id);
    }

    #[test]
    fn it_starts_a_session() {
        let session = Session::new("New Session".into());
        let session_id = session.id.clone();
        let mut data = Data::new();
        data.add_session(session).unwrap();

        let session = data.sessions.get_mut(&session_id).unwrap();
        session.start().unwrap();

        assert_eq!(State::Started, session.state);
    }

    #[test]
    fn it_stops_a_session() {
        let session = Session::new("New Session".into());
        let session_id = session.id.clone();
        let mut data = Data::new();
        data.add_session(session).unwrap();

        let session = data.sessions.get_mut(&session_id).unwrap();
        session.start().unwrap();
        session.stop().unwrap();

        assert_eq!(State::Stopped, session.state);
    }
}
