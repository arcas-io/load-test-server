use crate::error::{Result, ServerError};
use crate::helpers::elapsed;
use crate::log::LogLevel;
use crate::peer_connection::{PeerConnectionManager, VideoReceiveState, VideoSendState};
// use crate::stats::{get_peer_connection_stats, get_stats, PeerConnectionStats, Stats};
use crate::stats::{get_stats, Stats};
use crate::webrtc_pool::WebRTCPool;
use core::fmt;
use dashmap::mapref::one::Ref;
use dashmap::DashMap;
use libwebrtc::empty_frame_producer::EmptyFrameProducer;
use libwebrtc::video_track_source::VideoTrackSource;
use log::{error, info};
use std::time::{Duration, SystemTime};

pub(crate) type PeerConnections = DashMap<String, PeerConnectionManager>;

impl From<PeerConnectionState> for crate::server::webrtc::PeerConnectionState {
    fn from(
        peer_connection_state: PeerConnectionState,
    ) -> crate::server::webrtc::PeerConnectionState {
        crate::server::webrtc::PeerConnectionState {
            num_sending: peer_connection_state.num_sending,
            num_not_sending: peer_connection_state.num_not_sending,
            num_receiving: peer_connection_state.num_receiving,
            num_not_receiving: peer_connection_state.num_not_receiving,
        }
    }
}

#[derive(Debug, Clone, PartialEq, strum::ToString)]
pub(crate) enum SessionState {
    Created,
    Started,
    Stopped,
}

#[derive(Debug, Default)]
pub(crate) struct PeerConnectionState {
    num_sending: i32,
    num_not_sending: i32,
    num_receiving: i32,
    num_not_receiving: i32,
}

pub(crate) struct Session {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) peer_connections: PeerConnections,
    pub(crate) video_source: VideoTrackSource,
    pub(crate) polling_state_s: Duration,
    pub(crate) log_level: LogLevel,
    pub(crate) state: SessionState,
    pub(crate) start_time: Option<SystemTime>,
    pub(crate) stop_time: Option<SystemTime>,
    pub(crate) webrtc_pool: WebRTCPool,
    frame_producer: EmptyFrameProducer,
}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "id={}, name={}, num_peer_connections={}, state={:?}, polling_state_s={:?}, log_level={:?}, start_time={:?}, stop_time={:?}",
            self.id,
            self.name,
            self.peer_connections.len(),
            self.state,
            self.polling_state_s,
            self.log_level,
            self.start_time,
            self.stop_time
        )
    }
}

impl Session {
    pub(crate) fn new(
        id: String,
        name: String,
        polling_state_s: Duration,
        log_level: LogLevel,
    ) -> Result<Self> {
        LogLevel::set_log_level(&log_level);
        let peer_connections: PeerConnections = DashMap::new();
        let (video_source, frame_producer) = PeerConnectionManager::file_video_source()?;
        let webrtc_pool = WebRTCPool::new(num_cpus::get())?;

        Ok(Self {
            id,
            name,
            peer_connections,
            video_source,
            state: SessionState::Created,
            polling_state_s,
            log_level,
            start_time: None,
            stop_time: None,
            frame_producer,
            webrtc_pool,
        })
    }

    pub(crate) fn start(&mut self) -> Result<()> {
        info!("Attempting to start session {}", self.id);

        if self.state != SessionState::Created {
            return Err(ServerError::InvalidStateError(
                "Only a created session can be started".into(),
            ));
        }

        self.state = SessionState::Started;
        self.start_time = Some(SystemTime::now());

        info!("Started session: {:?}", self);

        Ok(())
    }

    pub(crate) fn stop(&mut self) -> Result<()> {
        info!("Attempting to stop session {}", self.id);

        if self.state != SessionState::Started {
            return Err(ServerError::InvalidStateError(
                "Only a started session can be stopped".into(),
            ));
        }

        self.state = SessionState::Stopped;
        self.stop_time = Some(SystemTime::now());

        info!("stopped session: {:?}", self);

        drop(self);

        Ok(())
    }

    pub(crate) async fn export_peer_connection_stats(&self, should_poll_state: bool) {
        for mut pc in self.peer_connections.iter_mut() {
            pc.value_mut()
                .export_stats(&self.id, should_poll_state)
                .await
                .map_err(|e| error!("Failed to export stats for peer connection: {}", e))
                .ok();
        }
    }

    // Tally the states of all of the peer connections
    pub(crate) fn peer_connection_states(&self) -> PeerConnectionState {
        let mut peer_connection_state = PeerConnectionState::default();

        self.peer_connections.iter().for_each(|pc| {
            match pc.value().state.video_send {
                VideoSendState::Sending(_) => peer_connection_state.num_sending += 1,
                VideoSendState::NotSending(_) => peer_connection_state.num_not_sending += 1,
            };
            match pc.value().state.video_receive {
                VideoReceiveState::Receiving(_) => peer_connection_state.num_receiving += 1,
                VideoReceiveState::NotReceiving(_) => peer_connection_state.num_not_receiving += 1,
            };
        });

        peer_connection_state
    }

    pub(crate) async fn get_stats(&self) -> Result<Stats> {
        info!("Attempting to get stats for session {}", self.id);

        let stats = get_stats(self).await?;

        info!("Stats for session {}: {:?}", self.id, stats);

        Ok(stats)
    }

    pub(crate) fn add_peer_connection(&self, peer_connection: PeerConnectionManager) -> Result<()> {
        info!(
            "Attempting to add peer connection {} for session {}",
            peer_connection.id, self.id
        );
        let peer_connection_id = peer_connection.id.clone();

        self.peer_connections
            .insert(peer_connection_id.clone(), peer_connection);

        info!(
            "Added peer connection {} to session {}",
            &peer_connection_id, &self.id
        );

        Ok(())
    }

    pub(crate) fn get_peer_connection(
        &self,
        id: &str,
    ) -> Result<Ref<String, PeerConnectionManager>> {
        info!(
            "Attempting to get peer connection {} for session {}",
            id, self.id
        );

        let value = self.peer_connections.get(id).ok_or_else(|| {
            ServerError::InvalidPeerConnection(format!("Peer connection {} not found", id))
        })?;
        Ok(value)
    }

    // pub(crate) async fn get_peer_connection_stats(&self, id: &str) -> Result<PeerConnectionStats> {
    //     info!(
    //         "Attempting to get peer connection stats for session {} pc {}",
    //         self.id, id
    //     );

    //     let peer_connection = self.get_peer_connection(id)?;
    //     let video_sender_stats = peer_connection.get_stats().await?;
    //     let stats = video_sender_stats.into();

    //     info!(
    //         "Stats for session {} pc {}: {:?}",
    //         self.id, id, video_sender_stats
    //     );

    //     Ok(stats)
    // }

    pub(crate) fn elapsed_time(&self) -> Option<u64> {
        match self.state {
            SessionState::Created => None,
            SessionState::Started => elapsed(self.start_time, Some(SystemTime::now())),
            SessionState::Stopped => elapsed(self.start_time, self.stop_time),
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.frame_producer.cancel();
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
            .get_mut(&$session_id.clone())
            .ok_or_else(|| crate::error::ServerError::InvalidSessionError($session_id.to_string()))?
            .$fn($($args),*)
    };
}

#[macro_export]
macro_rules! get_session_attribute {
    ($shared_state:expr, $session_id:expr, $attr:ident) => {
        $shared_state
            .data
            .sessions
            .get(&$session_id)
            .ok_or_else(|| crate::error::ServerError::InvalidSessionError($session_id.to_string()))?
            .$attr
    };
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::data::Data;
    use crate::peer_connection::tests::new_peer_connection;
    use nanoid::nanoid;

    pub(crate) fn new_session() -> (String, Data) {
        let session = Session::new(
            nanoid!(),
            "New Session".into(),
            Duration::from_secs(1),
            LogLevel::None,
        )
        .unwrap();
        let session_id = session.id.clone();
        let data = Data::new();
        data.add_session(session).unwrap();
        (session_id, data)
    }

    #[test]
    fn it_adds_a_session() {
        let (session_id, data) = new_session();
        assert_eq!(session_id, data.sessions.get(&session_id).unwrap().id);
    }

    #[test]
    fn it_starts_a_session() {
        let (session_id, data) = new_session();
        let session = &mut *data.sessions.get_mut(&session_id).unwrap();
        session.start().unwrap();

        assert_eq!(SessionState::Started, session.state);
    }

    #[test]
    fn it_stops_a_session() {
        let (session_id, data) = new_session();
        let session = &mut *data.sessions.get_mut(&session_id).unwrap();
        session.start().unwrap();
        session.stop().unwrap();

        assert_eq!(SessionState::Stopped, session.state);
    }

    #[tokio::test]
    async fn it_exports_peer_connection_stats() {
        // tracing_subscriber::fmt::init();
        let (session_id, data) = new_session();
        let session = &mut *data.sessions.get_mut(&session_id).unwrap();
        session.start().unwrap();

        let pc = new_peer_connection().0;
        session.add_peer_connection(pc).unwrap();
        session.export_peer_connection_stats(true).await;

        // TODO: come up with an assertion, just testing we don't get an err
    }

    #[tokio::test]
    async fn it_gets_stats() {
        let (session_id, data) = new_session();
        let session = &mut *data.sessions.get_mut(&session_id).unwrap();
        session.start().unwrap();
        let stats = session.get_stats().await;

        // TODO: come up with a better assertion
        assert!(stats.is_ok());
    }

    #[test]
    fn it_creates_a_peer_connection() {
        tracing_subscriber::fmt::init();
        let (session_id, data) = new_session();
        let session = &mut *data.sessions.get_mut(&session_id).unwrap();
        session.start().unwrap();

        let pc = new_peer_connection().0;
        let pc_id = pc.id.clone();
        session.add_peer_connection(pc).unwrap();

        assert_eq!(session.peer_connections.get(&pc_id).unwrap().id, pc_id);
    }
}
