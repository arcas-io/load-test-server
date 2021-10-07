use crate::data::SharedState;
use crate::peer_connection::PeerConnectionQueueInner;
use crate::server::webrtc;
use crate::session::Session;
use crate::ServerError;
use crate::{call_session, peer_connection};
use libwebrtc::sdp::SessionDescription;
use log::info;
use tonic::{Request, Response, Status};
use webrtc::web_rtc_server::WebRtc;
use webrtc::{
    CreatePeerConnectionRequest, CreatePeerConnectionResponse, CreateSdpRequest, CreateSdpResponse,
    CreateSessionRequest, CreateSessionResponse, Empty, GetStatsRequest, GetStatsResponse,
    SetSdpRequest, SetSdpResponse, StartSessionRequest, StopSessionRequest,
};

impl From<webrtc::SdpType> for libwebrtc::sdp::SdpType {
    fn from(sdp_type: webrtc::SdpType) -> Self {
        match sdp_type {
            webrtc::SdpType::Offer => Self::Offer,
            webrtc::SdpType::Pranswer => Self::PrAnswer,
            webrtc::SdpType::Answer => Self::Answer,
            webrtc::SdpType::Rollback => Self::Rollback,
        }
    }
}

#[tonic::async_trait]
impl WebRtc for SharedState {
    async fn create_session(
        &self,
        request: Request<CreateSessionRequest>,
    ) -> std::result::Result<Response<CreateSessionResponse>, Status> {
        info!("{:?}", request);

        let name = request.into_inner().name;
        let session = Session::new(name);
        let session_id = session.id.clone();
        self.data.add_session(session)?;
        let reply = webrtc::CreateSessionResponse { session_id };

        Ok(Response::new(reply))
    }

    async fn start_session(
        &self,
        request: Request<StartSessionRequest>,
    ) -> std::result::Result<Response<Empty>, Status> {
        info!("{:?}", request);

        let session_id = request.into_inner().session_id;
        call_session!(self, session_id, start)?;
        let reply = Empty {};

        Ok(Response::new(reply))
    }

    async fn stop_session(
        &self,
        request: Request<StopSessionRequest>,
    ) -> std::result::Result<Response<Empty>, Status> {
        info!("{:?}", request);

        let session_id = request.into_inner().session_id;
        call_session!(self, session_id, stop)?;
        let reply = webrtc::Empty {};

        Ok(Response::new(reply))
    }

    async fn get_stats(
        &self,
        request: Request<GetStatsRequest>,
    ) -> std::result::Result<Response<GetStatsResponse>, Status> {
        info!("{:?}", request);

        let session_id = request.into_inner().session_id;
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

        Ok(Response::new(reply))
    }

    async fn create_peer_connection(
        &self,
        request: Request<CreatePeerConnectionRequest>,
    ) -> std::result::Result<Response<CreatePeerConnectionResponse>, Status> {
        info!("{:?}", request);

        let request = request.into_inner();
        let CreatePeerConnectionRequest { name, session_id } = request;
        let peer_connection_id = nanoid::nanoid!();
        let peer_connection = crate::peer_connection::PeerConnection::new(
            &self.peer_connection_factory,
            peer_connection_id.clone(),
            name.clone(),
        )?;

        call_session!(self, session_id, add_peer_connection, peer_connection).await?;

        let reply = webrtc::CreatePeerConnectionResponse { peer_connection_id };

        Ok(Response::new(reply))
    }

    async fn create_offer(
        &self,
        request: Request<CreateSdpRequest>,
    ) -> Result<tonic::Response<CreateSdpResponse>, tonic::Status> {
        let request = request.into_inner();
        let offer = {
            let session = &*(self
                .data
                .sessions
                .get(&request.session_id)
                .ok_or_else(|| ServerError::InvalidSessionError(request.session_id.clone()))?);
            let peer_connection = &mut *session
                .peer_connections
                .get_mut(&request.peer_connection_id)
                .ok_or_else(|| {
                    ServerError::InvalidPeerConnection(request.peer_connection_id.clone())
                })?;
            peer_connection.webrtc_peer_connection.create_offer()
        };
        match offer {
            Err(_) => Err(tonic::Status::internal("could not create offer")),
            Ok(sdp) => Ok(Response::new(webrtc::CreateSdpResponse {
                sdp: sdp.to_string(),
                sdp_type: webrtc::SdpType::Offer.into(),
                session_id: request.session_id,
                peer_connection_id: request.peer_connection_id,
            })),
        }
    }

    async fn create_answer(
        &self,
        request: Request<CreateSdpRequest>,
    ) -> Result<tonic::Response<CreateSdpResponse>, tonic::Status> {
        let request = request.into_inner();
        let answer = {
            let session = &*(self
                .data
                .sessions
                .get(&request.session_id)
                .ok_or_else(|| ServerError::InvalidSessionError(request.session_id.clone()))?);
            let peer_connection = &mut *session
                .peer_connections
                .get_mut(&request.peer_connection_id)
                .ok_or_else(|| {
                    ServerError::InvalidPeerConnection(request.peer_connection_id.clone())
                })?;
            peer_connection.webrtc_peer_connection.create_answer()
        };
        match answer {
            Err(_) => Err(tonic::Status::internal("could not create answer")),
            Ok(sdp) => Ok(Response::new(webrtc::CreateSdpResponse {
                sdp: sdp.to_string(),
                session_id: request.session_id,
                peer_connection_id: request.peer_connection_id,
                sdp_type: webrtc::SdpType::Answer.into(),
            })),
        }
    }

    async fn set_local_description(
        &self,
        request: Request<SetSdpRequest>,
    ) -> Result<tonic::Response<SetSdpResponse>, tonic::Status> {
        let request = request.into_inner();
        let session_id = request.session_id.clone();
        let peer_connection_id = request.peer_connection_id.clone();
        let sdp = SessionDescription::from_string(request.sdp_type().into(), request.sdp)
            .map_err(|_| tonic::Status::invalid_argument("could not parse sdp"))?;
        let session = &*(self
            .data
            .sessions
            .get(&session_id)
            .ok_or_else(|| ServerError::InvalidSessionError(session_id.clone()))?);
        let peer_connection = &mut *session
            .peer_connections
            .get_mut(&peer_connection_id)
            .ok_or_else(|| ServerError::InvalidPeerConnection(peer_connection_id.clone()))?;
        peer_connection
            .webrtc_peer_connection
            .set_local_description(sdp)
            .map_err(|_| tonic::Status::internal("could not set sdp"))?;

        Ok(Response::new(SetSdpResponse {
            session_id,
            peer_connection_id,
            success: true,
        }))
    }

    async fn set_remote_description(
        &self,
        request: Request<SetSdpRequest>,
    ) -> Result<tonic::Response<SetSdpResponse>, tonic::Status> {
        let request = request.into_inner();
        let session_id = request.session_id.clone();
        let peer_connection_id = request.peer_connection_id.clone();
        let sdp = SessionDescription::from_string(request.sdp_type().into(), request.sdp)
            .map_err(|_| tonic::Status::invalid_argument("could not parse sdp"))?;
        let session = &*(self
            .data
            .sessions
            .get(&session_id.clone())
            .ok_or_else(|| ServerError::InvalidSessionError(session_id.clone()))?);
        let peer_connection = &mut *session
            .peer_connections
            .get_mut(&peer_connection_id)
            .ok_or_else(|| ServerError::InvalidPeerConnection(peer_connection_id.clone()))?;
        peer_connection
            .webrtc_peer_connection
            .set_remote_description(sdp)
            .map_err(|_| tonic::Status::internal("could not set sdp"))?;

        Ok(Response::new(SetSdpResponse {
            session_id,
            peer_connection_id,
            success: true,
        }))
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
