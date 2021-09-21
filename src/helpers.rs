use prost_types::Timestamp;
use std::time::SystemTime;
use tracing::error;

// calculate elapsed time
pub(crate) fn elapsed(
    start_time: Option<SystemTime>,
    stop_time: Option<SystemTime>,
) -> Option<u64> {
    if let (Some(start_time), Some(stop_time)) = (start_time, stop_time) {
        return stop_time
            .duration_since(start_time)
            .map_err(|e| error!("{}", e.to_string()))
            .map(|duration| duration.as_secs())
            .ok();
    }

    None
}

// convert system time to timestamp
pub(crate) fn systemtime_to_timestamp(time: Option<SystemTime>) -> Option<Timestamp> {
    time.and_then(|time| Some(Timestamp::from(time)))
}
