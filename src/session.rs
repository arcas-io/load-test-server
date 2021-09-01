use crate::error::Result;
use crate::server::{webrtc, MyWebRtc};
use libwebrtc::peerconnection::PeerConnection as LibWebRtcPeerConnection;
use nanoid::nanoid;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tonic::{Request, Response, Status};
use tracing::info;
use webrtc::web_rtc_server::WebRtc;
use webrtc::{CreateSessionRequest, CreateSessionResponse};

#[derive(Debug)]
pub(crate) struct PeerConnection {
    id: String,
    session_id: String,
    name: String,
    internal_peer_connection: LibWebRtcPeerConnection,
}

#[derive(Debug)]
pub(crate) struct Session {
    id: String,
    name: String,
    peer_connections: PeerConnections,
}

impl Session {
    pub(crate) fn new(name: String) -> Self {
        let id = nanoid!();
        let peer_connections: PeerConnections = HashMap::new();

        Self {
            id,
            name,
            peer_connections,
        }
    }
}

pub(crate) type SessionStorage = HashMap<String, Session>;
pub(crate) type PeerConnections = HashMap<String, PeerConnection>;

#[tonic::async_trait]
impl WebRtc for MyWebRtc {
    async fn create_session(
        &self,
        request: Request<CreateSessionRequest>,
    ) -> std::result::Result<Response<CreateSessionResponse>, Status> {
        info!("{:?}", request);

        let name = request.into_inner().name;
        let session_id = add_session(name, self.sessions.clone())?;
        let reply = webrtc::CreateSessionResponse { session_id };

        Ok(Response::new(reply))
    }
}

// Add a new session to sessions (in internal state)
fn add_session(name: String, sessions: Arc<Mutex<SessionStorage>>) -> Result<String> {
    let session = Session::new(name);
    let session_id = session.id.clone();

    info!("Added session: {:?}", session);

    sessions.lock()?.insert(session_id.clone(), session);

    Ok(session_id)
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::session::SessionStorage;
    use std::sync::{Arc, Mutex};

    #[test]
    fn it_adds_a_session() {
        let session_storage = SessionStorage::new();
        let sessions = Arc::new(Mutex::new(session_storage));
        let session_id = add_session("New Session".into(), sessions.clone()).unwrap();

        assert_eq!(
            session_id,
            sessions.lock().unwrap().get(&session_id).unwrap().id
        );
    }

    // TODO: add int tests, running the server in a lazy static (if possible)
    // #[tokio::test]
    // async fn it_creates_a_session() {
    //     tokio::task::spawn(async {
    //         let addr = "[::1]:50051";
    //         serve(addr).await.unwrap();
    //     });
    // }
}
