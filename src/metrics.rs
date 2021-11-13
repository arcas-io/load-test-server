use crate::config::CONFIG;

use lazy_static::lazy_static;
use libwebrtc_sys::ffi::{ArcasVideoReceiverStats, ArcasVideoSenderStats};

lazy_static! {
    static ref METRICS: dogstatsd::Client = {
        let opts = dogstatsd::Options {
            to_addr: format!("{}:{}", CONFIG.statsd_host, CONFIG.statsd_port),
            ..Default::default()
        };
        dogstatsd::Client::new(opts).unwrap()
    };
}

pub fn write_video_rx_stats(stat: &ArcasVideoReceiverStats, pc_id: &str, sess_id: &str) {
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

pub fn write_video_tx_stats(stat: &ArcasVideoSenderStats, pc_id: &str, sess_id: &str) {
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
