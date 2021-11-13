use crate::error::Result;
use crate::metrics::{write_video_rx_stats, write_video_tx_stats};

use core::fmt;
use libwebrtc::peer_connection::{PeerConnection, PeerConnectionConfig, PeerConnectionFactory};
use libwebrtc::raw_video_frame_producer::{GStreamerRawFrameProducer, RawFrameProducer};
use libwebrtc::sdp::{SDPType, SessionDescription};
use libwebrtc::transceiver::{TransceiverInit, VideoTransceiver};
use libwebrtc::video_codec::VideoCodec;
use libwebrtc::video_track::VideoTrack;
use libwebrtc::video_track_source::VideoTrackSource;
use libwebrtc_sys::ffi::ArcasVideoSenderStats;

use tracing::warn;

// TODO: temp allowing dead code, only used in tests currently
#[allow(dead_code)]
pub(crate) struct PeerConnectionManager {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) webrtc_peer_connection: PeerConnection,
}

impl fmt::Debug for PeerConnectionManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "id={}, name={}", self.id, self.name)
    }
}

impl PeerConnectionManager {
    pub(crate) fn new(
        peer_connection_factory: &PeerConnectionFactory,
        id: String,
        name: String,
    ) -> Result<PeerConnectionManager> {
        let webrtc_peer_connection =
            peer_connection_factory.create_peer_connection(PeerConnectionConfig::default())?;

        let pc = PeerConnectionManager {
            id,
            name,
            webrtc_peer_connection,
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
        let value = self.webrtc_peer_connection.create_offer().await?;
        Ok(value)
    }

    pub(crate) async fn create_answer(&self) -> Result<SessionDescription> {
        let value = self.webrtc_peer_connection.create_answer().await?;
        Ok(value)
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
        peer_connection_factory: &PeerConnectionFactory,
        video_source: &VideoTrackSource,
        label: String,
    ) -> Result<VideoTrack> {
        let value = peer_connection_factory.create_video_track(label, video_source)?;
        Ok(value)
    }

    pub(crate) async fn add_track(
        &self,
        peer_connection_factory: &PeerConnectionFactory,
        video_source: &VideoTrackSource,
        label: String,
    ) -> Result<()> {
        let track = Self::create_track(peer_connection_factory, video_source, label)?;
        Ok(self
            .webrtc_peer_connection
            .add_video_track(vec!["0".into()], track)
            .await?)
    }

    pub(crate) async fn add_transceiver(
        &self,
        peer_connection_factory: &PeerConnectionFactory,
        video_source: &VideoTrackSource,
        label: String,
    ) -> Result<VideoTransceiver> {
        let init = TransceiverInit::new(
            vec!["0".into()],
            libwebrtc::transceiver::TransceiverDirection::SendOnly,
        );
        let track = Self::create_track(peer_connection_factory, video_source, label)?;
        let value = self
            .webrtc_peer_connection
            .add_video_transceiver(init, track)
            .await?;
        Ok(value)
    }

    // stream a pre-encoded file from gstreamer to avoid encoding overhead
    pub(crate) fn file_video_source() -> Result<(VideoTrackSource, GStreamerRawFrameProducer)> {
        let (source, source_writer) = VideoTrackSource::create();
        let codec = VideoCodec::vp9_default();
        let mut producer =GStreamerRawFrameProducer::new(
            "filesrc location=static/file.mp4 ! qtdemux name=demux demux.video_0 ! avdec_h264 ! videoconvert ! videoscale".into(), &codec)?;
        let rx = producer.start()?;

        std::thread::spawn(move || {
            while let Ok(frame) = rx.recv() {
                match source_writer.push_raw_frame(frame) {
                    Ok(_) => {}
                    Err(err) => {
                        warn!("error pushing frame: {}", err);
                    }
                }
            }
        });

        Ok((source, producer))
    }

    pub(crate) async fn export_stats(&self, session_id: &str) -> Result<()> {
        let session_id = session_id.to_string();
        let pc_id = self.id.clone();
        let stats = self.webrtc_peer_connection.get_stats().await?;

        for stat in &stats.video_receiver_stats {
            write_video_rx_stats(stat, &pc_id, &session_id);
        }

        for stat in &stats.video_sender_stats {
            write_video_tx_stats(stat, &pc_id, &session_id);
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {

    use super::*;
    use libwebrtc::{factory::Factory, video_track_source::VideoTrackSourceWriter};
    use nanoid::nanoid;
    use tokio::time::{sleep, Duration};

    pub(crate) fn peer_connection_params() -> (
        Factory,
        PeerConnectionFactory,
        (VideoTrackSource, VideoTrackSourceWriter),
    ) {
        let factory = Factory::new();
        let peer_factory = factory.create_peer_connection_factory().unwrap();
        let source = VideoTrackSource::create();
        (factory, peer_factory, source)
    }

    #[tokio::test]
    async fn it_creates_a_new_peer_connection() {
        let (_api, factory, mut _video_source) = peer_connection_params();
        PeerConnectionManager::new(&factory, nanoid!(), "new".into()).unwrap();
    }

    #[tokio::test]
    async fn it_gets_stats_for_a_peer_connection() {
        let (_api, factory, mut _video_source) = peer_connection_params();
        let pc = PeerConnectionManager::new(&factory, nanoid!(), "new".into()).unwrap();
        pc.get_stats().await.unwrap();
    }

    #[tokio::test]
    async fn it_exports_stats_for_a_peer_connection() {
        let session_id = nanoid!();
        let (_api, factory, _video_source) = peer_connection_params();
        let (video_source, _video_writer) = PeerConnectionManager::file_video_source().unwrap();
        let mut pc = PeerConnectionManager::new(&factory, nanoid!(), "new".into()).unwrap();
        pc.add_track(&factory, &video_source, "Testlabel".into())
            .await
            .unwrap();
        let offer = pc.create_offer().await.unwrap();
        pc.set_local_description(offer.get_type(), offer.to_string())
            .await
            .unwrap();

        let mut pc_recv =
            PeerConnectionManager::new(&factory, nanoid!(), "new_recv".into()).unwrap();
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

        let pc_cand = pc
            .webrtc_peer_connection
            .take_ice_candidate_rx()
            .unwrap()
            .recv()
            .await
            .unwrap();
        let pc_recv_cand = pc_recv
            .webrtc_peer_connection
            .take_ice_candidate_rx()
            .unwrap()
            .recv()
            .await
            .unwrap();

        pc.webrtc_peer_connection
            .add_ice_candidate(pc_recv_cand)
            .await
            .unwrap();
        pc_recv
            .webrtc_peer_connection
            .add_ice_candidate(pc_cand)
            .await
            .unwrap();

        sleep(Duration::from_millis(500)).await;
        pc.export_stats(&session_id.to_owned()).await.unwrap();
        pc_recv.export_stats(&session_id.to_owned()).await.unwrap();
        sleep(Duration::from_millis(200)).await;
    }

    #[tokio::test]
    async fn it_creates_an_offer() {
        let (_api, factory, mut _video_source) = peer_connection_params();
        let pc = PeerConnectionManager::new(&factory, nanoid!(), "new".into()).unwrap();
        pc.create_offer().await.unwrap();
    }

    #[tokio::test]
    async fn it_creates_an_answer() {
        let (_api, factory, mut _video_source) = peer_connection_params();
        let pc = PeerConnectionManager::new(&factory, nanoid!(), "new".into()).unwrap();
        let offer = pc.create_offer().await.unwrap();
        pc.set_remote_description(offer.get_type(), offer.to_string())
            .await
            .unwrap();
        pc.create_answer().await.unwrap();
    }

    #[tokio::test]
    async fn it_sets_local_description() {
        let (_api, factory, mut _video_source) = peer_connection_params();
        let pc = PeerConnectionManager::new(&factory, nanoid!(), "new".into()).unwrap();
        let offer = pc.create_offer().await.unwrap();
        pc.set_local_description(offer.get_type(), offer.to_string())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn it_sets_remote_description() {
        let (_api, factory, mut _video_source) = peer_connection_params();
        let pc = PeerConnectionManager::new(&factory, nanoid!(), "new".into()).unwrap();
        let offer = pc.create_offer().await.unwrap();
        pc.set_remote_description(offer.get_type(), offer.to_string())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn it_adds_a_track() {
        let (_api, factory, (video_source, _)) = peer_connection_params();
        let pc = PeerConnectionManager::new(&factory, nanoid!(), "new".into()).unwrap();
        pc.add_track(&factory, &video_source, "Testlabel".into())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn it_adds_a_transceiver() {
        let (_api, factory, (video_source, _)) = peer_connection_params();
        let pc = PeerConnectionManager::new(&factory, nanoid!(), "new".into()).unwrap();
        pc.add_transceiver(&factory, &video_source, "Testlabel".into())
            .await
            .unwrap();
    }
}
