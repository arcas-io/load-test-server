use crate::error::{Result, ServerError};
use crate::helpers::systemtime_to_timestamp;
use crate::session::{Session, State};
use libwebrtc_sys::ffi::ArcasVideoSenderStats;
use std::time::SystemTime;

#[derive(Debug)]
pub(crate) struct SessionStats {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) num_peer_connections: u64,
    pub(crate) state: State,
    pub(crate) start_time: Option<SystemTime>,
    pub(crate) stop_time: Option<SystemTime>,
    pub(crate) elapsed_time: u64,
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
            elapsed_time: session.elapsed_time().unwrap_or(0),
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
pub(crate) struct PeerConnectionStats {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) video_sender: Vec<ArcasVideoSenderStats>,
}

impl From<PeerConnectionStats> for crate::server::webrtc::PeerConnectionStats {
    fn from(
        peer_connection_stats: PeerConnectionStats,
    ) -> crate::server::webrtc::PeerConnectionStats {
        crate::server::webrtc::PeerConnectionStats {
            id: peer_connection_stats.id.clone(),
            name: peer_connection_stats.name.clone(),
            video_sender: peer_connection_stats
                .video_sender
                .into_iter()
                .map(|stats| stats.into())
                .collect(),
        }
    }
}

impl From<ArcasVideoSenderStats> for crate::server::webrtc::PeerConnectionStat {
    fn from(
        video_sender_stats: ArcasVideoSenderStats,
    ) -> crate::server::webrtc::PeerConnectionStat {
        crate::server::webrtc::PeerConnectionStat {
            ssrc: video_sender_stats.ssrc,
            packets_sent: video_sender_stats.packets_sent,
            bytes_sent: video_sender_stats.bytes_sent,
            frames_encoded: video_sender_stats.frames_encoded,
            key_frames_encoded: video_sender_stats.key_frames_encoded,
            total_encode_time: video_sender_stats.total_encode_time,
            frame_width: video_sender_stats.frame_width,
            frame_height: video_sender_stats.frame_height,
            retransmitted_packets_sent: video_sender_stats.retransmitted_packets_sent,
            retransmitted_bytes_sent: video_sender_stats.retransmitted_bytes_sent,
            total_packet_send_delay: video_sender_stats.total_packet_send_delay,
            nack_count: video_sender_stats.nack_count,
            fir_count: video_sender_stats.fir_count,
            pli_count: video_sender_stats.pli_count,
            quality_limitation_reason: video_sender_stats.quality_limitation_reason,
            quality_limitation_resolution_changes: video_sender_stats
                .quality_limitation_resolution_changes,
            remote_packets_lost: video_sender_stats.remote_packets_lost,
            remote_jitter: video_sender_stats.remote_jitter,
            remote_round_trip_time: video_sender_stats.remote_round_trip_time,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Stats {
    pub(crate) session: SessionStats,
    pub(crate) peer_connections: Vec<PeerConnectionStats>,
}

pub(crate) async fn get_stats(session: &Session) -> Result<Stats> {
    let keys: Vec<String> = session
        .peer_connections
        .iter()
        .map(|p| p.key().clone())
        .collect();
    let mut peer_connections = vec![];

    for peer_connection_id in keys {
        // take the peer connection out of the hashmap so that stats can be
        // pulled out of it (cannot just use a reference)
        let session_id = session.id.clone();
        let peer_id = peer_connection_id.clone();
        let peer_connection = session
            .peer_connections
            .remove(&peer_connection_id)
            .ok_or_else(|| ServerError::GetStatsError(session_id, peer_id))?
            .1;

        // get the peer connection's stats
        let video_sender = peer_connection.get_stats().await?;
        let peer_connection_stats = PeerConnectionStats {
            id: peer_connection.id.clone(),
            name: peer_connection.name.clone(),
            video_sender,
        };
        peer_connections.push(peer_connection_stats);

        // put the peer connection back into the hashmap
        session
            .add_peer_connection(peer_connection)
            .map_err(|_e| ServerError::GetStatsError(session.id.clone(), peer_connection_id))?;
    }

    let stats = Stats {
        session: session.into(),
        peer_connections,
    };

    Ok(stats)
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::data::Data;
    use std::{thread, time::Duration};

    #[tokio::test]
    async fn it_gets_stats() {
        let session = Session::new("New Session".into()).unwrap();
        let session_id = session.id.clone();
        let data = Data::new();
        data.add_session(session).unwrap();

        let session = &mut *data.sessions.get_mut(&session_id).unwrap();
        session.start().unwrap();

        thread::sleep(Duration::from_millis(1000));
        let stats = get_stats(session).await.unwrap();
        assert_eq!(1, stats.session.elapsed_time);

        thread::sleep(Duration::from_millis(1000));
        session.stop().unwrap();

        let stats = get_stats(session).await.unwrap();
        println!("{:#?}", stats);
        assert_eq!(2, stats.session.elapsed_time);
    }
}
