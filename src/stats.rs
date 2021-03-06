use crate::error::Result;
use crate::helpers::systemtime_to_timestamp;

use crate::session::{PeerConnectionState, Session, SessionState};
use libwebrtc::peer_connection::PeerConnectionStats;
// use libwebrtc_sys::ffi::ArcasVideoSenderStats;

use libwebrtc::transceiver::VideoTransceiver;
use std::time::SystemTime;

#[derive(Debug)]
pub(crate) struct SessionStats {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) num_peer_connections: u64,
    pub(crate) state: SessionState,
    pub(crate) peer_connection_state: PeerConnectionState,
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
            peer_connection_state: session.peer_connection_states(),
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
            peer_connection_state: Some(session.peer_connection_state.into()),
            start_time: systemtime_to_timestamp(session.start_time),
            stop_time: systemtime_to_timestamp(session.stop_time),
            elapsed_time: session.elapsed_time,
        }
    }
}

// impl From<PeerConnectionStats> for crate::server::webrtc::PeerConnectionStats {
//     fn from(
//         peer_connection_stats: PeerConnectionStats,
//     ) -> crate::server::webrtc::PeerConnectionStats {
//         crate::server::webrtc::PeerConnectionStats {
//             id: peer_connection_stats.id.clone(),
//             name: peer_connection_stats.name.clone(),
//             video_sender: peer_connection_stats
//                 .video_sender
//                 .into_iter()
//                 .map(|stats| stats.into())
//                 .collect(),
//         }
//     }
// }

// impl From<ArcasVideoSenderStats> for PeerConnectionStats {
//     fn from(
//         video_sender_stats: ArcasVideoSenderStats,
//     ) -> crate::server::webrtc::PeerConnectionStats {
//         crate::server::webrtc::PeerConnectionStats {
//             ssrc: video_sender_stats.ssrc,
//             packets_sent: video_sender_stats.packets_sent,
//             bytes_sent: video_sender_stats.bytes_sent,
//             frames_encoded: video_sender_stats.frames_encoded,
//             key_frames_encoded: video_sender_stats.key_frames_encoded,
//             total_encode_time: video_sender_stats.total_encode_time,
//             frame_width: video_sender_stats.frame_width,
//             frame_height: video_sender_stats.frame_height,
//             retransmitted_packets_sent: video_sender_stats.retransmitted_packets_sent,
//             retransmitted_bytes_sent: video_sender_stats.retransmitted_bytes_sent,
//             total_packet_send_delay: video_sender_stats.total_packet_send_delay,
//             nack_count: video_sender_stats.nack_count,
//             fir_count: video_sender_stats.fir_count,
//             pli_count: video_sender_stats.pli_count,
//             quality_limitation_reason: video_sender_stats.quality_limitation_reason,
//             quality_limitation_resolution_changes: video_sender_stats
//                 .quality_limitation_resolution_changes,
//             remote_packets_lost: video_sender_stats.remote_packets_lost,
//             remote_jitter: video_sender_stats.remote_jitter,
//             remote_round_trip_time: video_sender_stats.remote_round_trip_time,
//         }
//     }
// }

#[derive(Debug)]
pub(crate) struct Stats {
    pub(crate) session: SessionStats,
}

pub(crate) async fn get_stats(session: &Session) -> Result<Stats> {
    let stats = Stats {
        session: session.into(),
    };

    Ok(stats)
}

pub(crate) async fn _get_video_transceiver_stats(
    tscv: &VideoTransceiver,
) -> Result<PeerConnectionStats> {
    Ok(tscv.get_stats().await?)
}

// pub(crate) async fn get_peer_connection_stats(
//     peer_connection: &PeerConnectionManager,
// ) -> Result<PeerConnectionStats> {
//     let video_sender = peer_connection.get_stats().await?;
//     let peer_connection_stats = PeerConnectionStats {
//         id: peer_connection.id.clone(),
//         name: peer_connection.name.clone(),
//         video_sender,
//     };

//     Ok(peer_connection_stats)
// }

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    // use crate::data::Data;
    // use crate::peer_connection::tests::{new_peer_connection, peer_connection_params};
    // use crate::peer_connection::PeerConnectionManager;
    use crate::session::tests::new_session;
    // use crate::webrtc_pool::WebRTCPool;
    // use bytes::Bytes;
    // use libwebrtc::peer_connection::PeerConnectionStats;
    // use libwebrtc::video_frame::{EmptyVideoFrame, RawVideoFrame};
    // use libwebrtc::video_track_source::{VideoTrackSource, VideoTrackSourceWriter};
    use libwebrtc_sys::ffi::ArcasVideoSenderStats;
    // use nanoid::nanoid;
    // use std::fmt::Debug;
    use std::{thread, time::Duration};
    // use tracing_subscriber::fmt::Formatter;

    pub(crate) fn video_receiver_stats() -> ArcasVideoSenderStats {
        ArcasVideoSenderStats {
            ssrc: 0,
            packets_sent: 0,
            bytes_sent: 0,
            frames_encoded: 0,
            key_frames_encoded: 0,
            total_encode_time: 0.0,
            frame_width: 0,
            frame_height: 0,
            retransmitted_packets_sent: 0,
            retransmitted_bytes_sent: 0,
            total_packet_send_delay: 0.0,
            nack_count: 0,
            fir_count: 0,
            pli_count: 0,
            quality_limitation_reason: 0, // 0 - kNone, 1 - kCpu, 2 - kBandwidth, 3 - kOther
            quality_limitation_resolution_changes: 0,
            remote_packets_lost: 0,
            remote_jitter: 0.0,
            remote_round_trip_time: 0.0,
        }
    }

    #[tokio::test]
    async fn it_gets_stats() {
        let (session_id, data) = new_session();
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

    // pub(crate) fn new_peer_connection() -> (
    //     PeerConnectionManager,
    //     WebRTCPool,
    //     (VideoTrackSource, VideoTrackSourceWriter),
    // ) {
    //     let (pool, video_source) = peer_connection_params();
    //     let pc;
    //     {
    //         let factory = pool.factory_list.get(&0).unwrap();
    //         pc = PeerConnectionManager::new(
    //             &factory.peer_connection_factory,
    //             0,
    //             nanoid!(),
    //             "new".into(),
    //         )
    //         .unwrap();
    //     }
    //     (pc, pool, video_source)
    // }

    // #[tokio::test]
    // async fn it_gets_transceiver_stats() {
    //     let (session_id, data) = new_session();
    //     let session = &mut *data.sessions.get_mut(&session_id).unwrap();
    //     session.start().unwrap();
    //     let (mut pc, pool, video_source) = new_peer_connection();
    //     let tscv = pc
    //         .add_transceiver(&pool, &video_source.0, "Testlabel".into())
    //         .await
    //         .unwrap();
    //     pc.add_track(&pool, &video_source.0, "A track".into());
    //     video_source
    //         .1
    //         .push_empty_frame(EmptyVideoFrame::create(1642022184u64).unwrap());
    //     video_source
    //         .1
    //         .push_raw_frame(RawVideoFrame::create(1, 1, 1642022335, Bytes::from("!")).unwrap());
    //     thread::sleep(Duration::from_millis(1000));
    //     let stats = get_video_transceiver_stats(&tscv).await.unwrap();
    //     let a = &stats;
    //     assert_eq!(0, stats.video_receiver_stats.len());
    // }
}
