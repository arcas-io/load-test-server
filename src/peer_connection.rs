use crate::error::Result;
use crate::metrics::{write_video_rx_stats, write_video_tx_stats};
use crate::webrtc_pool::WebRTCPool;

use core::fmt;
use libwebrtc::empty_frame_producer::EmptyFrameProducer;
use libwebrtc::encoded_video_frame_producer::DEFAULT_FPS;
use libwebrtc::error::WebRTCError;

use libwebrtc::ice_candidate::ICECandidate;
use libwebrtc::peer_connection::{
    PeerConnection, PeerConnectionConfig, PeerConnectionFactory, VideoReceiverStats,
    VideoSenderStats,
};
use libwebrtc::peer_connection_observer::ConnectionState;
use libwebrtc::sdp::{SDPType, SessionDescription};
use libwebrtc::transceiver::{AudioTransceiver, TransceiverInit, VideoTransceiver};

use libwebrtc::video_track::VideoTrack;
use libwebrtc::video_track_source::VideoTrackSource;
use libwebrtc_sys::ffi::ArcasVideoSenderStats;

use tokio::sync::mpsc::Receiver;

use tracing::warn;

// Store the last bytes_sent in the enum
#[derive(Debug, PartialEq)]
pub(crate) enum VideoSendState {
    Sending(u64),
    NotSending(u64),
}

// Store the last bytes_received in the enum
#[derive(Debug, PartialEq)]
pub(crate) enum VideoReceiveState {
    Receiving(u64),
    NotReceiving(u64),
}

#[derive(Debug, PartialEq)]
pub(crate) struct PeerConnectionState {
    pub(crate) video_send: VideoSendState,
    pub(crate) video_receive: VideoReceiveState,
}
// TODO: temp allowing dead code, only used in tests currently
#[allow(dead_code)]
pub(crate) struct PeerConnectionManager {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) webrtc_peer_connection: PeerConnection,
    pub(crate) pool_id: u32,
    pub(crate) state: PeerConnectionState,
}

impl fmt::Debug for PeerConnectionManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "id={}, name={}", self.id, self.name)
    }
}

impl PeerConnectionManager {
    pub(crate) fn new(
        peer_connection_factory: &PeerConnectionFactory,
        pool_id: u32,
        id: String,
        name: String,
    ) -> Result<PeerConnectionManager> {
        let webrtc_peer_connection =
            peer_connection_factory.create_peer_connection(PeerConnectionConfig::default())?;

        let pc = PeerConnectionManager {
            id,
            name,
            webrtc_peer_connection,
            pool_id,
            state: PeerConnectionState {
                video_send: VideoSendState::NotSending(0),
                video_receive: VideoReceiveState::NotReceiving(0),
            },
        };

        Ok(pc)
    }

    /// Send the callback to the rust ffi bindings and just listen for the first message.
    ///
    /// If the message fails, just return an empty vec.
    pub(crate) async fn get_stats(&self) -> Result<Vec<ArcasVideoSenderStats>> {
        let stats = self.webrtc_peer_connection.get_stats().await?;
        Ok(stats.video_sender_stats)
    }

    pub(crate) async fn create_offer(&self) -> Result<SessionDescription> {
        let offer = self.webrtc_peer_connection.create_offer().await?;
        Ok(offer)
    }

    pub(crate) async fn create_answer(&self) -> Result<SessionDescription> {
        let answer = self.webrtc_peer_connection.create_answer().await?;
        Ok(answer)
    }

    pub(crate) async fn set_local_description(&self, sdp_type: SDPType, sdp: String) -> Result<()> {
        let sdp = SessionDescription::new(sdp_type, sdp)?;
        Ok(self
            .webrtc_peer_connection
            .set_local_description(sdp)
            .await?)
    }

    pub(crate) async fn set_remote_description(
        &self,
        sdp_type: SDPType,
        sdp: String,
    ) -> Result<()> {
        let sdp = SessionDescription::new(sdp_type, sdp)?;
        Ok(self
            .webrtc_peer_connection
            .set_remote_description(sdp)
            .await?)
    }

    /// NOTE: This is *not* async as media calls are generally intended to be run syncrhonously within
    /// libwebrtc.
    fn create_track(
        pool_id: u32,
        pool: &WebRTCPool,
        video_source: &VideoTrackSource,
        label: String,
    ) -> Result<VideoTrack> {
        let peer_connection_factory = pool.factory_list.get(&pool_id).ok_or_else(|| {
            WebRTCError::UnexpectedError(format!("unknown factory id: {}", &pool_id))
        })?;
        let value = peer_connection_factory
            .value()
            .peer_connection_factory
            .create_video_track(label, video_source)?;
        Ok(value)
    }

    pub(crate) async fn add_track(
        &self,
        pool: &WebRTCPool,
        video_source: &VideoTrackSource,
        label: String,
    ) -> Result<()> {
        let track = Self::create_track(self.pool_id, pool, video_source, label)?;
        Ok(self
            .webrtc_peer_connection
            .add_video_track(vec!["0".into()], track)
            .await?)
    }

    pub(crate) async fn add_transceiver(
        &self,
        pool: &WebRTCPool,
        video_source: &VideoTrackSource,
        label: String,
    ) -> Result<VideoTransceiver> {
        let init = TransceiverInit::new(
            vec!["0".into()],
            libwebrtc::transceiver::TransceiverDirection::SendOnly,
        );
        let track = Self::create_track(self.pool_id, pool, video_source, label)?;
        let value = self
            .webrtc_peer_connection
            .add_video_transceiver(init, track)
            .await?;
        Ok(value)
    }

    // stream a pre-encoded file from gstreamer to avoid encoding overhead
    pub(crate) fn file_video_source() -> Result<(VideoTrackSource, EmptyFrameProducer)> {
        let (source, source_writer) = VideoTrackSource::create();
        // The empty frame producer ensures we receive the right messages from
        // the encoder factory without actually sending any frames. These
        // "empty" frames are most importantly not allocating I420 color space
        // buffers so are very cheap to generate.
        let mut producer = EmptyFrameProducer::new(DEFAULT_FPS)?;
        let rx = producer.start()?;
        let frame = rx.recv().unwrap();
        source_writer.push_empty_frame(frame).unwrap();

        std::thread::spawn(move || {
            while let Ok(frame) = rx.recv() {
                match source_writer.push_empty_frame(frame) {
                    Ok(_) => {}
                    Err(err) => {
                        warn!("error pushing frame: {}", err);
                    }
                }
            }
        });

        Ok((source, producer))
    }

    // Export stats
    pub(crate) async fn export_stats(&mut self, session_id: String) -> Result<()> {
        let pc_id = self.id.clone();
        let stats = self.webrtc_peer_connection.get_stats().await?;

        for stat in &stats.video_receiver_stats {
            log::trace!("{:?}", stat);
            self.set_receive_state(stat);
            write_video_rx_stats(stat, &pc_id, &session_id);
        }

        for stat in &stats.video_sender_stats {
            log::trace!("{:?}", stat);
            self.set_send_state(stat);
            write_video_tx_stats(stat, &pc_id, &session_id);
        }
        Ok(())
    }

    pub fn connection_state_rx(&mut self) -> Result<Receiver<ConnectionState>> {
        Ok(self.webrtc_peer_connection.take_connection_state_rx()?)
    }

    pub fn ice_candidates_rx(&mut self) -> Result<Receiver<ICECandidate>> {
        Ok(self.webrtc_peer_connection.take_ice_candidate_rx()?)
    }

    pub fn video_track_rx(&mut self) -> Result<Receiver<VideoTransceiver>> {
        Ok(self.webrtc_peer_connection.take_video_track_rx()?)
    }

    pub(crate) async fn get_transceivers(&self) -> (Vec<VideoTransceiver>, Vec<AudioTransceiver>) {
        self.webrtc_peer_connection.get_transceivers()
    }

    /// Set the send state for a peer connection
    pub(crate) fn set_send_state(&mut self, video_sender_stats: &VideoSenderStats) {
        self.state.video_send = if self.is_sending(video_sender_stats.bytes_sent) {
            VideoSendState::Sending(video_sender_stats.bytes_sent)
        } else {
            VideoSendState::NotSending(video_sender_stats.bytes_sent)
        };
    }

    /// Determine is the peer connection is send.
    /// Rule: is send if bytes_sent are incrementing
    fn is_sending(&self, bytes_sent: u64) -> bool {
        let last_bytes_sent = match self.state.video_send {
            VideoSendState::Sending(bytes_sent) => bytes_sent,
            VideoSendState::NotSending(bytes_sent) => bytes_sent,
        };

        bytes_sent > last_bytes_sent
    }

    /// Set the receiving state for a peer connection
    pub(crate) fn set_receive_state(&mut self, video_receiver_stats: &VideoReceiverStats) {
        self.state.video_receive = if self.is_receiving(video_receiver_stats.bytes_received) {
            VideoReceiveState::Receiving(video_receiver_stats.bytes_received)
        } else {
            VideoReceiveState::NotReceiving(video_receiver_stats.bytes_received)
        };
    }

    /// Determine is the peer connection is receiving.
    /// Rule: is receiving if bytes_received are incrementing
    fn is_receiving(&self, bytes_received: u64) -> bool {
        let last_bytes_received = match self.state.video_receive {
            VideoReceiveState::Receiving(bytes_received) => bytes_received,
            VideoReceiveState::NotReceiving(bytes_received) => bytes_received,
        };

        bytes_received > last_bytes_received
    }
}

#[cfg(test)]
pub(crate) mod tests {

    use super::*;
    use crate::stats::tests::video_receiver_stats;
    use libwebrtc::video_track_source::VideoTrackSourceWriter;
    use nanoid::nanoid;

    use tokio::time::{sleep, Duration};

    pub(crate) fn peer_connection_params(
    ) -> (WebRTCPool, (VideoTrackSource, VideoTrackSourceWriter)) {
        let pool = WebRTCPool::new(1).unwrap();
        let source = VideoTrackSource::create();
        (pool, source)
    }

    pub(crate) fn new_peer_connection() -> (
        PeerConnectionManager,
        WebRTCPool,
        (VideoTrackSource, VideoTrackSourceWriter),
    ) {
        let (pool, video_source) = peer_connection_params();
        let pc;
        {
            let factory = pool.factory_list.get(&0).unwrap();
            pc = PeerConnectionManager::new(
                &factory.peer_connection_factory,
                0,
                nanoid!(),
                "new".into(),
            )
            .unwrap();
        }
        (pc, pool, video_source)
    }

    #[tokio::test]
    async fn it_creates_a_new_peer_connection() {
        new_peer_connection();
    }

    #[tokio::test]
    async fn it_gets_and_exports_stats_for_a_peer_connection() {
        let session_id = nanoid!();
        let (mut pc, pool, _) = new_peer_connection();
        let (video_source, _video_writer) = PeerConnectionManager::file_video_source().unwrap();
        pc.add_track(&pool, &video_source, "Testlabel".into())
            .await
            .unwrap();
        let offer = pc.create_offer().await.unwrap();
        pc.set_local_description(offer.get_type(), offer.to_string())
            .await
            .unwrap();

        let mut pc_recv = new_peer_connection().0;
        pc_recv
            .set_remote_description(offer.get_type(), offer.to_string())
            .await
            .unwrap();
        let answer = pc_recv.create_answer().await.unwrap();
        pc_recv
            .set_local_description(answer.get_type(), answer.to_string())
            .await
            .unwrap();
        pc.set_remote_description(answer.get_type(), answer.to_string())
            .await
            .unwrap();

        let pc_cand = pc.ice_candidates_rx().unwrap().recv().await.unwrap();
        let pc_recv_cand = pc_recv.ice_candidates_rx().unwrap().recv().await.unwrap();
        pc.webrtc_peer_connection
            .add_ice_candidate(pc_recv_cand)
            .await
            .unwrap();
        pc_recv
            .webrtc_peer_connection
            .add_ice_candidate(pc_cand)
            .await
            .unwrap();

        let stats = pc.get_stats().await.unwrap();
        println!("{:?}", stats);

        sleep(Duration::from_millis(1000)).await;

        let stats = pc.get_stats().await.unwrap();
        println!("{:?}", stats);

        pc.export_stats(session_id.clone()).await.unwrap();
        pc_recv.export_stats(session_id.clone()).await.unwrap();
        sleep(Duration::from_millis(200)).await;
    }

    #[tokio::test]
    async fn it_creates_an_offer() {
        let pc = new_peer_connection().0;
        pc.create_offer().await.unwrap();
    }

    #[tokio::test]
    async fn it_creates_an_answer() {
        let pc = new_peer_connection().0;
        let offer = pc.create_offer().await.unwrap();
        pc.set_remote_description(offer.get_type(), offer.to_string())
            .await
            .unwrap();
        pc.create_answer().await.unwrap();
    }

    #[tokio::test]
    async fn it_sets_local_description() {
        let pc = new_peer_connection().0;
        let offer = pc.create_offer().await.unwrap();
        pc.set_local_description(offer.get_type(), offer.to_string())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn it_sets_remote_description() {
        let pc = new_peer_connection().0;
        let offer = pc.create_offer().await.unwrap();
        pc.set_remote_description(offer.get_type(), offer.to_string())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn it_adds_a_track() {
        let (pc, pool, video_source) = new_peer_connection();
        pc.add_track(&pool, &video_source.0, "Testlabel".into())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn it_adds_a_transceiver() {
        let (pc, pool, video_source) = new_peer_connection();
        pc.add_transceiver(&pool, &video_source.0, "Testlabel".into())
            .await
            .unwrap();
    }

    #[test]
    fn it_sets_sending_state() {
        let mut pc = new_peer_connection().0;
        let mut stats = video_receiver_stats();
        assert_eq!(pc.state.video_send, VideoSendState::NotSending(0));

        pc.set_send_state(&stats);
        assert_eq!(pc.state.video_send, VideoSendState::NotSending(0));

        stats.bytes_sent = 100;
        pc.set_send_state(&stats);
        assert_eq!(pc.state.video_send, VideoSendState::Sending(100));

        pc.set_send_state(&stats);
        assert_eq!(pc.state.video_send, VideoSendState::NotSending(100));
    }
}
