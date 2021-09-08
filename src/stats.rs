use crate::error::Result;
use crate::helpers::systemtime_to_timestamp;
use crate::session::{Session, State};
use std::time::SystemTime;

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

impl From<&Session> for SessionStats {
    fn from(session: &Session) -> SessionStats {
        SessionStats {
            id: session.id.clone(),
            name: session.name.clone(),
            num_peer_connections: session.peer_connections.len() as u64,
            state: session.state.clone(),
            start_time: session.start_time,
            stop_time: session.stop_time,
            elapsed_time: session.elapsed_time(),
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
            start_time: systemtime_to_timestamp(session.start_time),
            stop_time: systemtime_to_timestamp(session.stop_time),
            elapsed_time: session.elapsed_time,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Stats {
    pub(crate) session: SessionStats,
}

pub(crate) fn get_stats(session: &Session) -> Result<Stats> {
    let stats = Stats {
        session: session.into(),
    };

    // TODO: implement LibWebRtc here

    Ok(stats)
}
#[cfg(test)]
mod tests {

    use super::*;
    use crate::data::Data;
    use std::{thread, time::Duration};

    #[test]
    fn it_gets_stats() {
        let session = Session::new("New Session".into());
        let session_id = session.id.clone();
        let mut data = Data::new();
        data.add_session(session).unwrap();

        let session = data.sessions.get_mut(&session_id).unwrap();
        session.start().unwrap();

        thread::sleep(Duration::from_millis(1000));
        let stats = get_stats(&session).unwrap();
        assert_eq!(Some(1), stats.session.elapsed_time);

        thread::sleep(Duration::from_millis(1000));
        let stats = get_stats(&session).unwrap();
        assert_eq!(Some(2), stats.session.elapsed_time);

        session.stop().unwrap();

        let stats = get_stats(&session).unwrap();
        println!("{:#?}", stats);
        assert_eq!(Some(2), stats.session.elapsed_time);
    }
}
