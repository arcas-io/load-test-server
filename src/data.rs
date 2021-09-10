use crate::error::Result;
use crate::session::Session;
use std::collections::HashMap;
use tracing::info;

pub(crate) type Sessions = HashMap<String, Session>;

/// The in-memory persistent data structure for the server.
///
/// sessions: holds current and past sessions, keyed by session.id
#[derive(Debug)]
pub(crate) struct Data {
    pub(crate) sessions: Sessions,
}

impl Data {
    pub(crate) fn new() -> Self {
        Self {
            sessions: Sessions::new(),
        }
    }

    // Add a new session to sessions (in internal state)
    pub(crate) fn add_session(&mut self, session: Session) -> Result<()> {
        info!("Adding session: {:?}", session);

        self.sessions.insert(session.id.clone(), session);

        Ok(())
    }
}

/// Macro to remove boilderplate in the handlers when manipulating sessions
/// with data.
///
/// # Examples
///
/// ```
/// // Invoking a method on session with no parameters
/// call_session!(self.data, session_id, stop)?;
///
/// // Invoking an async method on session with 2 parameters
/// let peer_connection_id = call_session!(
///     self.data,
///     session_id.clone(),
///     add_peer_connection,
///     peer_connection_factory,
///     name
/// )
/// .await?;
/// ```
///
#[macro_export]
macro_rules! call_session {
    ($data:expr, $session_id:expr, $fn:ident $(, $args:expr)*) => {
        $data
            .lock()
            .await
            .sessions
            .get_mut(&$session_id)
            .ok_or_else(|| crate::error::ServerError::InvalidSessionError($session_id))?
            .$fn($($args),*)
    };
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn it_adds_a_session() {
        let session = Session::new("New Session".into());
        let session_id = session.id.clone();
        let mut data = Data::new();
        data.add_session(session).unwrap();
        let added_session = data.sessions.get(&session_id).unwrap();

        assert_eq!(session_id, added_session.id);
    }
}
