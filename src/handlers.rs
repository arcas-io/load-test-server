use crate::error;
use crate::data::SharedState;
use crate::peer_connection::PeerConnectionQueueInner;
use crate::server::webrtc;
use crate::session::Session;
use log::info;
use tonic::{Request, Response, Status};
use webrtc::web_rtc_server::WebRtc;
use webrtc::{
    CreatePeerConnectionRequest, CreatePeerConnectionResponse, CreateSessionRequest,
    CreateSessionResponse, Empty, GetStatsRequest, GetStatsResponse, StartSessionRequest,
    StopSessionRequest, CreateSdpRequest, CreateSdpResponse, SetSdpRequest, SetSdpResponse,
};

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
        {
            let session = &mut *self.data.sessions.get_mut(&session_id).ok_or(error::ServerError::InvalidSessionError(session_id))?;
            session.start()?;
        }
        let reply = Empty {};

        Ok(Response::new(reply))
    }

    async fn stop_session(
        &self,
        request: Request<StopSessionRequest>,
    ) -> std::result::Result<Response<Empty>, Status> {
        info!("{:?}", request);

        let session_id = request.into_inner().session_id;
        {
            let session = &mut *self.data.sessions.get_mut(&session_id).ok_or(error::ServerError::InvalidSessionError(session_id))?;
            session.stop()?;
        }
        let reply = webrtc::Empty {};

        Ok(Response::new(reply))
    }

    async fn get_stats(
        &self,
        request: Request<GetStatsRequest>,
    ) -> std::result::Result<Response<GetStatsResponse>, Status> {
        info!("{:?}", request);

        let session_id = request.into_inner().session_id;
        let stats = {
            let session = &mut *self.data.sessions.get_mut(&session_id).ok_or(error::ServerError::InvalidSessionError(session_id))?;
            session.get_stats().await?
        };
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

        // add to the peer connection queue, the websocket will consume this
        // queue and create the peer connection in libwebrtc
        let peer_connection_id = nanoid::nanoid!();
        let inner = PeerConnectionQueueInner {
            id: peer_connection_id.clone(),
            session_id,
            name,
        };
        {
            let mut pc_queue = self.peer_connection_queue.lock().await;
            pc_queue.push_back(inner);
        }

        let reply = webrtc::CreatePeerConnectionResponse { peer_connection_id };

        Ok(Response::new(reply))
    }

    async fn create_answer(&self, request: Request<CreateSdpRequest>,)->Result<tonic::Response<CreateSdpResponse>,tonic::Status> {
        let request = request.into_inner();
        {
            let session = & *(self.data.sessions.get(&request.session_id).ok_or(error::ServerError::InvalidSessionError(request.session_id.clone()))?);
            session.peer_connections.get(&request.peer_connection_id).ok_or(error::ServerError::InvalidPeerConnection(request.peer_connection_id.clone()))?;
        }
        todo!()
    }

    async fn set_local_description(&self, _request: Request<SetSdpRequest>,)->Result<tonic::Response<SetSdpResponse>,tonic::Status> {
        todo!()
    }

    async fn set_remote_description(&self, _request: Request<SetSdpRequest>,)->Result<tonic::Response<SetSdpResponse>,tonic::Status> {
        todo!()
    }

    async fn create_offer(&self, _request: Request<CreateSdpRequest>,)->Result<tonic::Response<CreateSdpResponse>,tonic::Status> {
        todo!()
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
