use std::ffi::c_void;

use lazy_static::lazy_static;
use libwebrtc::ffi::memory::{C_deallocate_owned_object, OwnedRustObject};
use libwebrtc::ffi::stats_collector::{Rs_VideoReceiverStats, Rs_VideoSenderStats};
use libwebrtc::stats_collector::RTCStatsCollectorCallbackTrait;

use prometheus::register_int_gauge_vec;
use prometheus::{register_gauge_vec, GaugeVec, IntGaugeVec, TextEncoder};
use warp::Rejection;

static VIDEO_LABELS: &[&str; 3] = &["pc_id", "sess_id", "ssrc"];

lazy_static! {
    static ref V_RX_PACKETS_RECEIVED: IntGaugeVec = register_int_gauge_vec!(
        "video_rx_packets_received",
        "Incoming packets received",
        VIDEO_LABELS,
    )
    .unwrap();
    static ref V_RX_PACKETS_LOST: IntGaugeVec = register_int_gauge_vec!(
        "video_rx_packets_lost",
        "Incoming packets lost",
        VIDEO_LABELS,
    )
    .unwrap();
    static ref V_RX_PACKETS_REPAIRED: IntGaugeVec = register_int_gauge_vec!(
        "video_rx_packets_repaired",
        "Incoming packets repaired",
        VIDEO_LABELS,
    )
    .unwrap();
    static ref V_RX_BYTES_RECEIVED: IntGaugeVec = register_int_gauge_vec!(
        "video_rx_bytes_received",
        "Incoming video bytes received",
        VIDEO_LABELS,
    )
    .unwrap();
    static ref V_RX_FRAMES_DECODED: IntGaugeVec = register_int_gauge_vec!(
        "video_rx_frames_decoded",
        "Incoming video frames decoded",
        VIDEO_LABELS,
    )
    .unwrap();
    static ref V_RX_KEYFRAMES_DECODED: IntGaugeVec = register_int_gauge_vec!(
        "video_rx_keyframes_decoded",
        "Incoming video keyframes decoded",
        VIDEO_LABELS,
    )
    .unwrap();
    static ref V_RX_FRAMES_DROPPED: IntGaugeVec = register_int_gauge_vec!(
        "video_rx_frames_dropped",
        "Incoming video frames dropped",
        VIDEO_LABELS,
    )
    .unwrap();
    static ref V_RX_TOTAL_DECODE_TIME: GaugeVec = register_gauge_vec!(
        "video_rx_total_decode_time",
        "Incoming video decode time",
        VIDEO_LABELS,
    )
    .unwrap();
    static ref V_RX_FRAME_WIDTH: IntGaugeVec = register_int_gauge_vec!(
        "video_rx_frame_width",
        "Incoming video frame width",
        VIDEO_LABELS,
    )
    .unwrap();
    static ref V_RX_FRAME_HEIGHT: IntGaugeVec = register_int_gauge_vec!(
        "video_rx_frame_height",
        "Incoming video frame height",
        VIDEO_LABELS,
    )
    .unwrap();
}

lazy_static! {
    static ref V_TX_PACKETS_SENT: IntGaugeVec = register_int_gauge_vec!(
        "video_tx_packets_sent",
        "Outgoing video packets sent",
        VIDEO_LABELS,
    )
    .unwrap();
    static ref V_TX_BYTES_SENT: IntGaugeVec = register_int_gauge_vec!(
        "video_tx_bytes_sent",
        "Outgoing video bytes sent",
        VIDEO_LABELS,
    )
    .unwrap();
    static ref V_TX_FRAMES_ENCODED: IntGaugeVec = register_int_gauge_vec!(
        "video_tx_frames_encoded",
        "Outgoing video frames encoded",
        VIDEO_LABELS,
    )
    .unwrap();
    static ref V_TX_KEYFRAMES_ENCODED: IntGaugeVec = register_int_gauge_vec!(
        "video_tx_keyframes_encoded",
        "Outgoing video keyframes encoded",
        VIDEO_LABELS,
    )
    .unwrap();
    static ref V_TX_TOTAL_ENCODE_TIME: GaugeVec = register_gauge_vec!(
        "video_tx_total_encode_time",
        "Outgoing video encode time",
        VIDEO_LABELS,
    )
    .unwrap();
    static ref V_TX_FRAME_WIDTH: IntGaugeVec = register_int_gauge_vec!(
        "video_tx_frame_width",
        "Outgoing video frame width",
        VIDEO_LABELS,
    )
    .unwrap();
    static ref V_TX_FRAME_HEIGHT: IntGaugeVec = register_int_gauge_vec!(
        "video_tx_frame_height",
        "Outgoing video frame height",
        VIDEO_LABELS,
    )
    .unwrap();
    static ref V_TX_RTX_PACKETS_SENT: IntGaugeVec = register_int_gauge_vec!(
        "video_tx_rtx_packets_sent",
        "Outgoing video retransmitted packets sent",
        VIDEO_LABELS,
    )
    .unwrap();
    static ref V_TX_RTX_BYTES_SENT: IntGaugeVec = register_int_gauge_vec!(
        "video_tx_rtx_bytes_sent",
        "Outgoing video retransmitted bytes sent",
        VIDEO_LABELS,
    )
    .unwrap();
}

macro_rules! set_int_gauge_vec_metric {
    ($metric_name:expr,$val:expr,$labels:expr) => {{
        let _ = ($metric_name)
            .get_metric_with_label_values($labels)
            .map(|s| s.set($val as i64));
    }};
}

macro_rules! set_gauge_vec_metric {
    ($metric_name:expr,$val:expr,$labels:expr) => {{
        let _ = ($metric_name)
            .get_metric_with_label_values($labels)
            .map(|s| s.set($val as f64));
    }};
}

// SLOW
fn write_video_rx_stats(stat: &Rs_VideoReceiverStats, pc_id: &str, sess_id: &str) {
    let ssrc = &stat.ssrc.to_string();
    let labels = &[pc_id, sess_id, &ssrc];
    set_int_gauge_vec_metric!(V_RX_PACKETS_RECEIVED, stat.packets_received, labels);
    set_int_gauge_vec_metric!(V_RX_PACKETS_LOST, stat.packets_lost, labels);
    set_int_gauge_vec_metric!(V_RX_PACKETS_REPAIRED, stat.packets_repaired, labels);
    set_int_gauge_vec_metric!(V_RX_BYTES_RECEIVED, stat.bytes_received, labels);
    set_int_gauge_vec_metric!(V_RX_FRAMES_DECODED, stat.frames_decoded, labels);
    set_int_gauge_vec_metric!(V_RX_KEYFRAMES_DECODED, stat.keyframes_decoded, labels);
    set_int_gauge_vec_metric!(V_RX_FRAMES_DROPPED, stat.frames_dropped, labels);
    set_gauge_vec_metric!(V_RX_TOTAL_DECODE_TIME, stat.total_decode_time, labels);
    set_int_gauge_vec_metric!(V_RX_FRAME_WIDTH, stat.frame_width, labels);
    set_int_gauge_vec_metric!(V_RX_FRAME_HEIGHT, stat.frame_height, labels);
}

// SLOW
fn write_video_tx_stats(stat: &Rs_VideoSenderStats, pc_id: &str, sess_id: &str) {
    let ssrc = &stat.ssrc.to_string();
    let labels = &[pc_id, sess_id, &ssrc];
    set_int_gauge_vec_metric!(V_TX_PACKETS_SENT, stat.packets_sent, labels);
    set_int_gauge_vec_metric!(V_TX_BYTES_SENT, stat.bytes_sent, labels);
    set_int_gauge_vec_metric!(V_TX_FRAMES_ENCODED, stat.frames_encoded, labels);
    set_int_gauge_vec_metric!(V_TX_KEYFRAMES_ENCODED, stat.key_frames_encoded, labels);
    set_gauge_vec_metric!(V_TX_TOTAL_ENCODE_TIME, stat.total_encode_time, labels);
    set_int_gauge_vec_metric!(V_TX_FRAME_WIDTH, stat.frame_width, labels);
    set_int_gauge_vec_metric!(V_TX_FRAME_HEIGHT, stat.frame_height, labels);
    set_int_gauge_vec_metric!(
        V_TX_RTX_PACKETS_SENT,
        stat.retransmitted_packets_sent,
        labels
    );
    set_int_gauge_vec_metric!(V_TX_RTX_BYTES_SENT, stat.retransmitted_bytes_sent, labels);
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

pub(crate) async fn metrics_handler() -> Result<impl warp::Reply, Rejection> {
    let encoder = TextEncoder::new();
    let metrics = prometheus::gather();
    match encoder.encode_to_string(&metrics) {
        Ok(res) => Ok(res),
        Err(_) => Ok("could not encode metrics".to_owned()),
    }
}
