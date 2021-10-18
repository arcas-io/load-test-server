use crate::error::{Result, ServerError};
use crate::helpers::elapsed;
use crate::peer_connection::{PeerConnection, PeerConnectionQueue};
use crate::stats::{get_stats, Stats};
use dashmap::DashMap;
use log::info;
use nanoid::nanoid;
use std::collections::VecDeque;
use std::time::SystemTime;

pub(crate) type PeerConnections = DashMap<String, PeerConnection>;

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
    pub(crate) peer_connection_queue: PeerConnectionQueue,
    pub(crate) state: State,
    pub(crate) start_time: Option<SystemTime>,
    pub(crate) stop_time: Option<SystemTime>,
}

impl Session {
    pub(crate) fn new(name: String) -> Self {
        let id = nanoid!();
        let peer_connections: PeerConnections = DashMap::new();
        let peer_connection_queue: PeerConnectionQueue = VecDeque::new();

        Self {
            id,
            name,
            peer_connections,
            peer_connection_queue,
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

    pub(crate) fn peer_connection_stats(&self) {
        self.peer_connections
            .iter()
            .for_each(|pc| pc.value().export_stats());
    }

    pub(crate) async fn get_stats(&mut self) -> Result<Stats> {
        info!("Attempting to get stats for session {}", self.id);

        let stats = get_stats(self).await?;

        info!("Stats for session {}: {:?}", self.id, stats);

        Ok(stats)
    }

    pub(crate) async fn add_peer_connection(
        &mut self,
        peer_connection: PeerConnection,
    ) -> Result<()> {
        let peer_connection_id = peer_connection.id.clone();

        self.peer_connections
            .insert(peer_connection_id.clone(), peer_connection);

        info!(
            "Added peer connection {} to session {}",
            &peer_connection_id, &self.id
        );

        Ok(())
    }

    pub(crate) fn elapsed_time(&self) -> Option<u64> {
        match self.state {
            State::Created => None,
            State::Started => elapsed(self.start_time, Some(SystemTime::now())),
            State::Stopped => elapsed(self.start_time, self.stop_time),
        }
    }
}

/// Macro to remove boilderplate in the handlers when manipulating sessions
/// with data.
///
/// # Examples
///
/// ```
/// // Invoking a method on session with no parameters
/// call_session!(self.data, session_id, stop)?;
///
/// // Invoking an async method on session with 2 parameters
/// let peer_connection_id = call_session!(
///     self,
///     session_id.clone(),
///     add_peer_connection,
///     peer_connection_factory,
///     name
/// )
/// .await?;
/// ```
///
#[macro_export]
macro_rules! call_session {
    ($shared_state:expr, $session_id:expr, $fn:ident $(, $args:expr)*) => {
        $shared_state
            .data
            .sessions
            .get_mut(&$session_id)
            .ok_or_else(|| crate::error::ServerError::InvalidSessionError($session_id))?
            .$fn($($args),*)
    };
}

#[cfg(test)]
mod tests {

    use libwebrtc::peerconnection_factory::PeerConnectionFactory;

    use super::*;
    use crate::data::Data;

    #[test]
    fn it_adds_a_session() {
        let session = Session::new("New Session".into());
        let session_id = session.id.clone();
        let data = Data::new();
        data.add_session(session).unwrap();

        assert_eq!(session_id, data.sessions.get(&session_id).unwrap().id);
    }

    #[test]
    fn it_starts_a_session() {
        let session = Session::new("New Session".into());
        let session_id = session.id.clone();
        let data = Data::new();
        data.add_session(session).unwrap();

        let session = &mut *data.sessions.get_mut(&session_id).unwrap();
        session.start().unwrap();

        assert_eq!(State::Started, session.state);
    }

    #[test]
    fn it_stops_a_session() {
        let session = Session::new("New Session".into());
        let session_id = session.id.clone();
        let data = Data::new();
        data.add_session(session).unwrap();

        let session = &mut *data.sessions.get_mut(&session_id).unwrap();
        session.start().unwrap();
        session.stop().unwrap();

        assert_eq!(State::Stopped, session.state);
    }

    #[tokio::test]
    async fn it_gets_stats() {
        let session = Session::new("New Session".into());
        let session_id = session.id.clone();
        let data = Data::new();
        data.add_session(session).unwrap();

        let session = &mut *data.sessions.get_mut(&session_id).unwrap();
        session.start().unwrap();
        let stats = session.get_stats().await;

        // TODO: come up with a better assertion
        assert!(stats.is_ok());
    }

    #[tokio::test]
    async fn it_creates_a_peer_connection() {
        tracing_subscriber::fmt::init();
        let factory = PeerConnectionFactory::new().unwrap();
        let session = Session::new("New Session".into());
        let session_id = session.id.clone();
        let data = Data::new();
        data.add_session(session).unwrap();

        let session = &mut *data.sessions.get_mut(&session_id).unwrap();
        session.start().unwrap();

        let pc_id = nanoid!();
        {
            let pc = PeerConnection::new(&factory.clone(), pc_id.clone(), session_id, "new".into())
                .unwrap();
            session.add_peer_connection(pc).await.unwrap();

            assert_eq!(session.peer_connections.get(&pc_id).unwrap().id, pc_id);
        }
    }
}
