use std::ffi::c_void;

use lazy_static::lazy_static;
use libwebrtc::ffi::memory::{C_deallocate_owned_object, OwnedRustObject};
use libwebrtc::ffi::stats_collector::{Rs_VideoReceiverStats, Rs_VideoSenderStats};
use libwebrtc::stats_collector::RTCStatsCollectorCallbackTrait;

lazy_static! {
    static ref STATSD_HOST: String = std::env::var("STATSD_HOST").unwrap_or("127.0.0.1".to_owned());
    static ref STATSD_PORT: String = std::env::var("STATSD_PORT").unwrap_or("9125".to_owned());
    static ref METRICS: dogstatsd::Client = {
        let mut opts = dogstatsd::Options::default();
        opts.to_addr = format!("{}:{}", *STATSD_HOST, *STATSD_PORT);
        dogstatsd::Client::new(opts).unwrap()
    };
}

fn write_video_rx_stats(stat: &Rs_VideoReceiverStats, pc_id: &str, sess_id: &str) {
    let tags = &[
        &format!("pc_id:{}", pc_id),
        &format!("sess_id:{}", sess_id),
        &format!("ssrc: {}", stat.ssrc),
    ];

    let _ = METRICS.count(
        "pc.video.rx.packets_received",
        stat.packets_received as i64,
        tags,
    );
    let _ = METRICS.count("pc.video.rx.packets_lost", stat.packets_lost as i64, tags);
    let _ = METRICS.count(
        "pc.video.rx.packets_repaired",
        stat.packets_repaired as i64,
        tags,
    );
    let _ = METRICS.count(
        "pc.video.rx.bytes_received",
        stat.bytes_received as i64,
        tags,
    );
    let _ = METRICS.count(
        "pc.video.rx.frames_decoded",
        stat.frames_decoded as i64,
        tags,
    );
    let _ = METRICS.count(
        "pc.video.rx.keyframes_decoded",
        stat.keyframes_decoded as i64,
        tags,
    );
    let _ = METRICS.count(
        "pc.video.rx.frames_dropped",
        stat.frames_dropped as i64,
        tags,
    );
    let _ = METRICS.gauge(
        "pc.video.rx.total_decode_time",
        stat.total_decode_time.to_string(),
        tags,
    );
    let _ = METRICS.gauge(
        "pc.video.rx.frame_width",
        stat.frame_width.to_string(),
        tags,
    );
    let _ = METRICS.gauge(
        "pc.video.rx.frame_height",
        stat.frame_height.to_string(),
        tags,
    );
}

fn write_video_tx_stats(stat: &Rs_VideoSenderStats, pc_id: &str, sess_id: &str) {
    let tags = [
        &format!("pc_id:{}", pc_id),
        &format!("sess_id:{}", sess_id),
        &format!("ssrc: {}", stat.ssrc),
    ];

    let _ = METRICS.count("pc.video.tx.packets_sent", stat.packets_sent as i64, tags);
    let _ = METRICS.count("pc.video.tx.bytes_sent", stat.bytes_sent as i64, tags);
    let _ = METRICS.count(
        "pc.video.tx.frames_encoded",
        stat.frames_encoded as i64,
        tags,
    );
    let _ = METRICS.count(
        "pc.video.tx.keyframes_encoded",
        stat.key_frames_encoded as i64,
        tags,
    );

    let _ = METRICS.gauge(
        "pc.video.tx.total_encode_time",
        stat.total_encode_time.to_string(),
        tags,
    );
    let _ = METRICS.gauge(
        "pc.video.tx.frame_width",
        stat.frame_width.to_string(),
        tags,
    );
    let _ = METRICS.gauge(
        "pc.video.tx.frame_height",
        stat.frame_height.to_string(),
        tags,
    );
    let _ = METRICS.gauge(
        "pc.video.tx.total_packet_send_delay",
        stat.total_packet_send_delay.to_string(),
        tags,
    );
    let _ = METRICS.gauge(
        "pc.video.tx.remote_jitter",
        stat.remote_jitter.to_string(),
        tags,
    );

    let _ = METRICS.count("pc.video.tx.nack_count", stat.nack_count as i64, tags);
    let _ = METRICS.count("pc.video.tx.fir_count", stat.fir_count as i64, tags);
    let _ = METRICS.count("pc.video.tx.pli_count", stat.pli_count as i64, tags);
    let _ = METRICS.count(
        "pc.video.tx.remote_packets_lost",
        stat.remote_packets_lost as i64,
        tags,
    );

    let _ = METRICS.gauge(
        "pc.video.tx.remote_round_trip_time",
        stat.remote_round_trip_time.to_string(),
        tags,
    );
}

// MetricsStatsCollector callback
// TODO: This is ***VERY*** inefficient. Find way to persist required
// metrics in peerconnection wrapper object
pub struct MetricsStatsCollectorCallback {
    pc_id: String,
    sess_id: String,
}

impl MetricsStatsCollectorCallback {
    pub fn new(peer_connection_id: String, session_id: String) -> Self {
        MetricsStatsCollectorCallback {
            pc_id: peer_connection_id,
            sess_id: session_id,
        }
    }
}

impl From<MetricsStatsCollectorCallback> for OwnedRustObject {
    fn from(collector: MetricsStatsCollectorCallback) -> Self {
        OwnedRustObject {
            object: Box::into_raw(Box::from(collector)) as *mut c_void,
            Deallocate: C_deallocate_owned_object::<MetricsStatsCollectorCallback>,
        }
    }
}

impl RTCStatsCollectorCallbackTrait for MetricsStatsCollectorCallback {
    fn on_stats_delivered(
        &mut self,
        video_receiver_stats: Vec<libwebrtc::ffi::stats_collector::Rs_VideoReceiverStats>,
        _audio_receiver_stats: Vec<libwebrtc::ffi::stats_collector::Rs_AudioReceiverStats>,
        video_sender_stats: Vec<libwebrtc::ffi::stats_collector::Rs_VideoSenderStats>,
        _audio_sender_stats: Vec<libwebrtc::ffi::stats_collector::Rs_AudioSenderStats>,
    ) {
        // TODO: This is ***VERY*** inefficient. Find way to persist required
        // metrics in peerconnection wrapper object
        for stat in &video_receiver_stats {
            write_video_rx_stats(stat, &self.pc_id, &self.sess_id);
        }

        for stat in &video_sender_stats {
            write_video_tx_stats(stat, &self.pc_id, &self.sess_id);
        }
    }
}
