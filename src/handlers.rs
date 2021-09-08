use crate::error::ServerError;
// use crate::get_session;
use crate::server::{webrtc, MyWebRtc};
use crate::session::Session;
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
        let session = Session::new(name);
        let session_id = session.id.clone();
        self.data
            .lock()
            .map_err(|e| ServerError::InternalError(e.to_string()))?
            .add_session(session)?;
        let reply = webrtc::CreateSessionResponse { session_id };

        Ok(Response::new(reply))
    }

    async fn start_session(
        &self,
        request: Request<StartSessionRequest>,
    ) -> std::result::Result<Response<Empty>, Status> {
        info!("{:?}", request);

        let session_id = request.into_inner().session_id;
        self.data
            .lock()
            .map_err(|e| ServerError::InternalError(e.to_string()))?
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| ServerError::InvalidSessionError(session_id))?
            .start()?;
        let reply = Empty {};

        Ok(Response::new(reply))
    }

    async fn stop_session(
        &self,
        request: Request<StopSessionRequest>,
    ) -> std::result::Result<Response<Empty>, Status> {
        info!("{:?}", request);

        let session_id = request.into_inner().session_id;
        self.data
            .lock()
            .map_err(|e| ServerError::InternalError(e.to_string()))?
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| ServerError::InvalidSessionError(session_id))?
            .stop()?;
        let reply = webrtc::Empty {};

        Ok(Response::new(reply))
    }

    async fn get_stats(
        &self,
        request: Request<GetStatsRequest>,
    ) -> std::result::Result<Response<GetStatsResponse>, Status> {
        info!("{:?}", request);

        let session_id = request.into_inner().session_id;
        let stats = self
            .data
            .lock()
            .map_err(|e| ServerError::InternalError(e.to_string()))?
            .sessions
            .get(&session_id)
            .ok_or_else(|| ServerError::InvalidSessionError(session_id))?
            .get_stats()?;
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
