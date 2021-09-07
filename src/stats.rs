use crate::error::{Result, ServerError};
use crate::session::{Session, SessionStorage, State};
use prost_types::Timestamp;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use tracing::{error, info, trace};

#[derive(Debug)]
pub(crate) struct SessionStats {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) num_peer_connections: u64,
    pub(crate) state: State,
    pub(crate) start_time: Option<SystemTime>,
    pub(crate) stop_time: Option<SystemTime>,
    pub(crate) elapsed_time: Option<u64>,
}

fn elapsed(start_time: Option<SystemTime>, stop_time: Option<SystemTime>) -> Option<u64> {
    if let (Some(start_time), Some(stop_time)) = (start_time, stop_time) {
        return stop_time
            .duration_since(start_time)
            .map_err(|e| error!("{}", e.to_string()))
            .map(|duration| duration.as_secs())
            .ok();
    }

    None
}

fn to_timestamp(time: Option<SystemTime>) -> Option<Timestamp> {
    if let Some(time) = time {
        return Some(Timestamp::from(time));
    }

    None
}

impl From<&Session> for SessionStats {
    fn from(session: &Session) -> SessionStats {
        let elapsed_time = match session.state {
            State::Created => None,
            State::Started => elapsed(session.start_time, Some(SystemTime::now())),
            State::Stopped => elapsed(session.start_time, session.stop_time),
        };

        SessionStats {
            id: session.id.clone(),
            name: session.name.clone(),
            num_peer_connections: session.peer_connections.len() as u64,
            state: session.state.clone(),
            start_time: session.start_time,
            stop_time: session.stop_time,
            elapsed_time,
        }
    }
}

impl From<SessionStats> for crate::server::webrtc::SessionStats {
    fn from(session: SessionStats) -> crate::server::webrtc::SessionStats {
        crate::server::webrtc::SessionStats {
            id: session.id,
            name: session.name,
            num_peer_connections: session.num_peer_connections,
            state: session.state.to_string(),
            start_time: to_timestamp(session.start_time),
            stop_time: to_timestamp(session.stop_time),
            elapsed_time: session.elapsed_time,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Stats {
    pub(crate) session: SessionStats,
}

pub(crate) fn get_stats(session_id: String, sessions: Arc<Mutex<SessionStorage>>) -> Result<Stats> {
    trace!("Getting stats for session {}", session_id);

    let sessions = sessions.lock()?;
    let session = sessions
        .get(&session_id.clone())
        .ok_or_else(|| ServerError::InvalidSessionError(session_id.clone()))?;
    let stats = Stats {
        session: session.into(),
    };

    // TODO: implement LibWebRtc here

    info!("Stats for session {} {:?}", session_id, stats);

    Ok(stats)
}
#[cfg(test)]
mod tests {

    use super::*;
    use crate::session::{add_session, start_session, stop_session, SessionStorage};
    use std::sync::{Arc, Mutex};
    use std::{thread, time::Duration};

    #[test]
    fn it_gets_stats() {
        let session_storage = SessionStorage::new();
        let sessions = Arc::new(Mutex::new(session_storage));
        let session_id = add_session("New Session".into(), sessions.clone()).unwrap();

        start_session(session_id.clone(), sessions.clone()).unwrap();

        thread::sleep(Duration::from_millis(1000));
        let stats = get_stats(session_id.clone(), sessions.clone()).unwrap();
        assert_eq!(Some(1), stats.session.elapsed_time);

        thread::sleep(Duration::from_millis(1000));
        let stats = get_stats(session_id.clone(), sessions.clone()).unwrap();
        assert_eq!(Some(2), stats.session.elapsed_time);

        stop_session(session_id.clone(), sessions.clone()).unwrap();

        let stats = get_stats(session_id, sessions).unwrap();
        assert_eq!(Some(2), stats.session.elapsed_time);
    }
}
