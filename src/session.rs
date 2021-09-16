use crate::error::{Result, ServerError};
use crate::helpers::elapsed;
use crate::peer_connection::PeerConnection;
use crate::stats::{get_stats, Stats};
use libwebrtc::peerconnection_factory::PeerConnectionFactory;
use log::info;
use nanoid::nanoid;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::Mutex;

pub(crate) type PeerConnections = HashMap<String, PeerConnection>;

#[derive(Debug, Clone, PartialEq, strum::ToString)]
pub(crate) enum State {
    Created,
    Started,
    Stopped,
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

    pub(crate) async fn add_peer_connection(
        &mut self,
        peer_connection_factory: Arc<Mutex<PeerConnectionFactory>>,
        name: String,
    ) -> Result<String> {
        info!(
            "Attempting to add a peer connection for session {}",
            self.id
        );

        let peer_connection = PeerConnection::new(peer_connection_factory, name).await?;
        let peer_connection_id = peer_connection.id.clone();

        self.peer_connections
            .insert(peer_connection_id.clone(), peer_connection);

        info!(
            "Added peer connection {} to session {}",
            self.id, peer_connection_id
        );

        Ok(peer_connection_id)
    }

    pub(crate) fn elapsed_time(&self) -> Option<u64> {
        match self.state {
            State::Created => None,
            State::Started => elapsed(self.start_time, Some(SystemTime::now())),
            State::Stopped => elapsed(self.start_time, self.stop_time),
        }
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

    #[test]
    fn it_gets_stats() {
        let session = Session::new("New Session".into());
        let session_id = session.id.clone();
        let mut data = Data::new();
        data.add_session(session).unwrap();

        let session = data.sessions.get_mut(&session_id).unwrap();
        session.start().unwrap();
        let stats = session.get_stats();

        // TODO: come up with a better assertion
        assert!(stats.is_ok());
    }

    #[tokio::test]
    async fn it_creates_a_peer_connection() {
        tracing_subscriber::fmt::init();
        let session = Session::new("New Session".into());
        let session_id = session.id.clone();
        let mut data = Data::new();
        data.add_session(session).unwrap();

        let session = data.sessions.get_mut(&session_id).unwrap();
        session.start().unwrap();

        let peer_connection_factory = Arc::new(Mutex::new(PeerConnectionFactory::new().unwrap()));
        let peer_connection_id = session
            .add_peer_connection(peer_connection_factory, "New Peer Connection".into())
            .await
            .unwrap();

        assert_eq!(
            session
                .peer_connections
                .get(&peer_connection_id)
                .unwrap()
                .id,
            peer_connection_id
        );
    }
}
