use crate::data::SharedState;
use crate::peer_connection::PeerConnectionManager;
use crate::server::webrtc;
use crate::session::Session;
use crate::{call_session, get_session_attribute};
use libwebrtc::sdp::SDPType;
use log::info;
use std::fmt::Debug;
use std::result::Result;
use tonic::{Request, Response, Status};
use webrtc::web_rtc_server::WebRtc;
use webrtc::{
    AddTrackRequest, AddTransceiverRequest, CreatePeerConnectionRequest,
    CreatePeerConnectionResponse, CreateSdpRequest, CreateSdpResponse, CreateSessionRequest,
    CreateSessionResponse, Empty, GetStatsRequest, GetStatsResponse, SetSdpRequest, SetSdpResponse,
    StartSessionRequest, StopSessionRequest,
};

// TODO: create a proc macro to inject requester and responder into each handler
fn requester<T: Debug>(tag: &str, request: Request<T>) -> T {
    let request = request.into_inner();
    info!("Request({}): {:?}", tag, request);
    request
}

fn responder<T: Debug>(tag: &str, response: T) -> Result<Response<T>, Status> {
    info!("Response({}): {:?}", tag, response);
    Ok(Response::new(response))
}

impl From<webrtc::SdpType> for SDPType {
    fn from(sdp_type: webrtc::SdpType) -> Self {
        match sdp_type {
            webrtc::SdpType::Offer => SDPType::Offer,
            webrtc::SdpType::Pranswer => SDPType::PrAnswer,
            webrtc::SdpType::Answer => SDPType::Answer,
            webrtc::SdpType::Rollback => SDPType::Rollback,
        }
    }
}

#[tonic::async_trait]
impl WebRtc for SharedState {
    async fn create_session(
        &self,
        request: Request<CreateSessionRequest>,
    ) -> Result<Response<CreateSessionResponse>, Status> {
        let CreateSessionRequest { session_id, name } = requester("create_session", request);
        let session = Session::new(session_id.clone(), name)?;
        self.data.add_session(session)?;
        let reply = webrtc::CreateSessionResponse { session_id };

        responder("create_session", reply)
    }

    async fn start_session(
        &self,
        request: Request<StartSessionRequest>,
    ) -> Result<Response<Empty>, Status> {
        let session_id = requester("start_session", request).session_id;
        call_session!(self, session_id, start)?;
        let reply = Empty {};

        responder("start_session", reply)
    }

    async fn stop_session(
        &self,
        request: Request<StopSessionRequest>,
    ) -> Result<Response<Empty>, Status> {
        let session_id = requester("stop_session", request).session_id;
        call_session!(self, session_id, stop)?;
        let reply = webrtc::Empty {};

        responder("stop_session", reply)
    }

    async fn get_stats(
        &self,
        request: Request<GetStatsRequest>,
    ) -> Result<Response<GetStatsResponse>, Status> {
        let session_id = requester("get_stats", request).session_id;
        let stats = call_session!(self, session_id, get_stats).await?;
        let peer_connections = stats
            .peer_connections
            .into_iter()
            .map(|peer_connection_stats| peer_connection_stats.into())
            .collect();
        let reply = webrtc::GetStatsResponse {
            session: Some(stats.session.into()),
            peer_connections,
        };

        responder("get_stats", reply)
    }

    async fn create_peer_connection(
        &self,
        request: Request<CreatePeerConnectionRequest>,
    ) -> Result<Response<CreatePeerConnectionResponse>, Status> {
        let CreatePeerConnectionRequest {
            session_id,
            peer_connection_id,
            name,
        } = requester("create_peer_connection", request);
        let peer_factory =
            &get_session_attribute!(self, session_id.clone(), peer_connection_factory);
        // create the peer connection
        let session = self.data.get_session(&session_id)?;
        let peer_connection =
            PeerConnectionManager::new(peer_factory, peer_connection_id.clone(), name.clone())?;

        // add the peer connection to the session
        session.add_peer_connection(peer_connection)?;
        let reply = webrtc::CreatePeerConnectionResponse { peer_connection_id };
        responder("create_peer_connection", reply)
    }

    async fn create_offer(
        &self,
        request: Request<CreateSdpRequest>,
    ) -> Result<tonic::Response<CreateSdpResponse>, tonic::Status> {
        let request = requester("create_offer", request);
        let session_id = request.session_id;
        let peer_connection_id = request.peer_connection_id;
        let session = self.data.get_session(&session_id)?;
        let pc = session.value().get_peer_connection(&peer_connection_id)?;
        let sdp = pc.value().create_offer().await?;

        let reply = CreateSdpResponse {
            sdp: sdp.to_string(),
            sdp_type: webrtc::SdpType::Offer.into(),
            session_id,
            peer_connection_id,
        };

        responder("create_offer", reply)
    }

    async fn create_answer(
        &self,
        request: Request<CreateSdpRequest>,
    ) -> Result<tonic::Response<CreateSdpResponse>, tonic::Status> {
        let request = requester("create_answer", request);
        let session_id = request.session_id;
        let peer_connection_id = request.peer_connection_id;
        let session = self.data.get_session(&session_id)?;
        let pc = session.value().get_peer_connection(&peer_connection_id)?;

        let sdp = pc.value().create_answer().await?;

        let reply = CreateSdpResponse {
            sdp: sdp.to_string(),
            sdp_type: webrtc::SdpType::Answer.into(),
            session_id,
            peer_connection_id,
        };

        responder("create_answer", reply)
    }

    async fn set_local_description(
        &self,
        request: Request<SetSdpRequest>,
    ) -> Result<tonic::Response<SetSdpResponse>, tonic::Status> {
        let request = requester("set_local_description", request);
        let sdp_type = request.sdp_type();
        let session_id = request.session_id;
        let peer_connection_id = request.peer_connection_id;
        let session = self.data.get_session(&session_id)?;
        let pc = session.value().get_peer_connection(&peer_connection_id)?;
        pc.value()
            .set_local_description(sdp_type.into(), request.sdp)
            .await?;

        let reply = SetSdpResponse {
            session_id,
            peer_connection_id,
            success: true,
        };

        responder("set_local_description", reply)
    }

    async fn set_remote_description(
        &self,
        request: Request<SetSdpRequest>,
    ) -> Result<tonic::Response<SetSdpResponse>, tonic::Status> {
        let request = requester("set_remote_description", request);
        let sdp_type = request.sdp_type();
        let sdp = request.sdp;
        let session_id = request.session_id;
        let peer_connection_id = request.peer_connection_id;
        let session = self.data.get_session(&session_id)?;
        let pc = session.value().get_peer_connection(&peer_connection_id)?;
        pc.value()
            .set_remote_description(sdp_type.into(), sdp)
            .await?;

        let reply = SetSdpResponse {
            session_id,
            peer_connection_id,
            success: true,
        };

        responder("set_remote_description", reply)
    }

    async fn add_track(
        &self,
        request: tonic::Request<AddTrackRequest>,
    ) -> Result<tonic::Response<Empty>, tonic::Status> {
        let request = requester("add_track", request);
        let session_id = request.session_id;
        let peer_connection_id = request.peer_connection_id;
        let _track_id = request.track_id;
        let track_label = request.track_label;
        let session = self.data.get_session(&session_id)?;
        let pc = session.value().get_peer_connection(&peer_connection_id)?;
        let video_source = &session.value().video_source;
        let peer_factory = &session.value().peer_connection_factory;

        pc.value()
            .add_track(peer_factory, video_source, track_label)
            .await?;

        let reply = Empty {};

        responder("add_track", reply)
    }

    async fn add_transceiver(
        &self,
        request: tonic::Request<AddTransceiverRequest>,
    ) -> Result<tonic::Response<Empty>, tonic::Status> {
        let request = requester("add_transceiver", request);
        let session_id = request.session_id;
        let peer_connection_id = request.peer_connection_id;
        let session = self.data.get_session(&session_id)?;
        let pc = session.value().get_peer_connection(&peer_connection_id)?;
        let _track_id = request.track_id;
        let track_label = if request.track_label.is_empty() {
            nanoid::nanoid!()
        } else {
            request.track_label
        };
        let peer_factory = &session.value().peer_connection_factory;
        let video_source = &session.value().video_source;
        pc.value()
            .add_transceiver(peer_factory, video_source, track_label)
            .await?;
        let reply = Empty {};

        responder("add_transceiver", reply)
    }
}

#[cfg(test)]
mod tests {

    // TODO: add int tests, running the server in a lazy static (if possible)
    // #[tokio::test]
    // async fn it_creates_a_session() {
    //     tokio::task::spawn(async {
    //         let addr = "[::1]:50051";
    //         serve(addr).await.unwrap();
    //     });
    // }
}
