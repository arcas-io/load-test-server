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

        // create a new session
        let name = request.into_inner().name;
        let session = Session::new(name);
        let session_id = session.id.clone();

        // add the new session to sessions in internal state
        add_session(session_id.clone(), session, self.sessions.clone())?;

        let reply = webrtc::CreateSessionResponse { session_id };

        Ok(Response::new(reply))
    }
}

fn add_session(
    session_id: String,
    session: Session,
    sessions: Arc<Mutex<SessionStorage>>,
) -> Result<()> {
    info!("Added session: {:?}", session);

    sessions.lock()?.insert(session_id, session);

    Ok(())
}
