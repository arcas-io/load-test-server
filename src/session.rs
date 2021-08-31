use crate::server::{webrtc, MyWebRtc};
use nanoid::nanoid;
use std::collections::HashMap;
use tonic::{Request, Response, Status};
use webrtc::web_rtc_server::WebRtc;
use webrtc::{CreateSessionRequest, CreateSessionResponse};

#[derive(Debug)]
pub(crate) struct PeerConnection {
    id: String,
    session_id: String,
    name: String,
    // internal_peer_connection: PeerConnectionFfi,
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
    ) -> Result<Response<CreateSessionResponse>, Status> {
        println!("{:?}", request);

        // create a new session
        let name = request.into_inner().name;
        let session = Session::new(name);
        let session_id = session.id.clone();

        // add the new session to &self.sessions
        &self
            .sessions
            .lock()
            .unwrap()
            .insert(session_id.clone(), session);

        let reply = webrtc::CreateSessionResponse { session_id };

        println!("{:?}", &self.sessions);

        Ok(Response::new(reply))
    }
}
