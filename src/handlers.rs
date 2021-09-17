use crate::call_session;
use crate::data::SharedState;
use crate::server::webrtc;
use crate::session::Session;
use log::info;
use tonic::{Request, Response, Status};
use webrtc::web_rtc_server::WebRtc;
use webrtc::{
    CreatePeerConnectionRequest, CreatePeerConnectionResponse, CreateSessionRequest,
    CreateSessionResponse, Empty, GetStatsRequest, GetStatsResponse, StartSessionRequest,
    StopSessionRequest,
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
        self.lock().await.data.add_session(session)?;
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
        let stats = call_session!(self, session_id, get_stats)?;
        let reply = webrtc::GetStatsResponse {
            session: Some(stats.session.into()),
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
        let peer_connection_factory = self.lock().await.peer_connection_factory.clone();
        let peer_connection_id = call_session!(
            self,
            session_id.clone(),
            add_peer_connection,
            peer_connection_factory,
            name
        )
        .await?;

        let reply = webrtc::CreatePeerConnectionResponse { peer_connection_id };

        Ok(Response::new(reply))
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
