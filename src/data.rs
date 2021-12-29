use std::sync::Arc;
use std::time::Duration;

use crate::error::{Result, ServerError};
use crate::session::Session;
use dashmap::mapref::one::Ref;
use dashmap::DashMap;
use log::info;

pub(crate) struct SharedState {
    pub(crate) data: Arc<Data>,
}

impl std::fmt::Debug for SharedState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedState")
            .field("data", &self.data)
            .finish()
    }
}

pub(crate) type Sessions = DashMap<String, Session>;

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
    pub(crate) fn add_session(&self, session: Session) -> Result<()> {
        info!("Adding session: {:?}", session);

        self.sessions.insert(session.id.clone(), session);

        Ok(())
    }

    pub(crate) fn get_session(&self, id: &str) -> Result<Ref<String, Session>> {
        let map = &self.sessions;
        let dashmap_value = map
            .get(id)
            .ok_or_else(|| ServerError::InvalidSessionError(id.into()))?;

        Ok(dashmap_value)
    }
}

impl SharedState {
    pub(crate) fn start_metrics_collection(&self) {
        let data = self.data.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            let mut elapsed = 1;

            interval.tick().await;

            loop {
                for session in &data.sessions {
                    let should_poll_state = elapsed % session.polling_state_s.as_secs() == 0;
                    log::warn!(
                        "should_poll_state: {}, elapsed: {}, polling_state_s: {}",
                        should_poll_state,
                        elapsed,
                        session.polling_state_s.as_secs()
                    );

                    session
                        .value()
                        .export_peer_connection_stats(should_poll_state)
                        .await;
                }

                // if a session exists, increment
                // if no session exists, restart elapsed
                if &data.sessions.len() != &0 {
                    elapsed += 1;
                } else {
                    elapsed = 1;
                }

                interval.tick().await;
            }
        });
    }
}

#[cfg(test)]
mod tests {

    use crate::session::tests::new_session;

    #[test]
    fn it_adds_and_gets_a_session() {
        let (session_id, data) = new_session();
        let added_session = data.get_session(&session_id).unwrap();

        assert_eq!(session_id, added_session.id);
    }
}
