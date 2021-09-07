use crate::server::{webrtc, MyWebRtc};
use crate::session::{add_session, start_session, stop_session};
use crate::stats::get_stats;
use tonic::{Request, Response, Status};
use tracing::info;
use webrtc::web_rtc_server::WebRtc;
use webrtc::{
    CreateSessionRequest, CreateSessionResponse, Empty, GetStatsRequest, GetStatsResponse,
    StartSessionRequest, StopSessionRequest,
};

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

    async fn start_session(
        &self,
        request: Request<StartSessionRequest>,
    ) -> std::result::Result<Response<Empty>, Status> {
        info!("{:?}", request);

        let session_id = request.into_inner().session_id;
        start_session(session_id, self.sessions.clone())?;
        let reply = Empty {};

        Ok(Response::new(reply))
    }

    async fn stop_session(
        &self,
        request: Request<StopSessionRequest>,
    ) -> std::result::Result<Response<Empty>, Status> {
        info!("{:?}", request);

        let session_id = request.into_inner().session_id;
        stop_session(session_id, self.sessions.clone())?;
        let reply = webrtc::Empty {};

        Ok(Response::new(reply))
    }

    async fn get_stats(
        &self,
        request: Request<GetStatsRequest>,
    ) -> std::result::Result<Response<GetStatsResponse>, Status> {
        info!("{:?}", request);

        let session_id = request.into_inner().session_id;
        let stats = get_stats(session_id, self.sessions.clone())?;
        let reply = webrtc::GetStatsResponse {
            session: Some(stats.session.into()),
        };

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
