use crate::server::webrtc;
use libwebrtc_sys::ffi::{set_arcas_log_level, LoggingSeverity};

#[derive(Debug)]
pub(crate) enum LogLevel {
    None,
    Info,
    Warn,
    Error,
    Verbose,
}

impl Default for LogLevel {
    fn default() -> Self {
        Self::Error
    }
}

impl LogLevel {
    pub(crate) fn set_log_level(log_level: &LogLevel) {
        set_arcas_log_level(log_level.into());
    }
}

impl Into<LoggingSeverity> for &LogLevel {
    fn into(self) -> LoggingSeverity {
        match self {
            LogLevel::None => LoggingSeverity::LS_NONE,
            LogLevel::Info => LoggingSeverity::LS_INFO,
            LogLevel::Warn => LoggingSeverity::LS_WARNING,
            LogLevel::Error => LoggingSeverity::LS_ERROR,
            LogLevel::Verbose => LoggingSeverity::LS_VERBOSE,
        }
    }
}

impl From<i32> for LogLevel {
    fn from(log_level: i32) -> Self {
        match webrtc::LogLevel::from_i32(log_level) {
            Some(webrtc::LogLevel::None) => LogLevel::None,
            Some(webrtc::LogLevel::Info) => LogLevel::Info,
            Some(webrtc::LogLevel::Warn) => LogLevel::Warn,
            Some(webrtc::LogLevel::Error) => LogLevel::Error,
            Some(webrtc::LogLevel::Verbose) => LogLevel::Verbose,
            None => LogLevel::None,
        }
    }
}
