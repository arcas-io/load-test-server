use crate::data::SharedState;
use crate::error::ServerError;
use crate::server::webrtc::{self};
use crate::session::Session;
use crate::{call_session, get_session_attribute};
use async_stream::stream;
use futures::Stream;
use libwebrtc::media_type::MediaType;
use libwebrtc::sdp::SDPType;
use libwebrtc::transceiver::{self, TransceiverDirection};
use log::{error, info};
use std::fmt::Debug;
use std::pin::Pin;
use std::result::Result;

use tokio::select;
use tonic::{Request, Response, Status};
use webrtc::web_rtc_server::WebRtc;
use webrtc::{
    AddTrackRequest, AddTransceiverRequest, CreatePeerConnectionRequest,
    CreatePeerConnectionResponse, CreateSdpRequest, CreateSdpResponse, CreateSessionRequest,
    CreateSessionResponse, Empty, GetStatsRequest, GetStatsResponse, PeerConnectionObserverMessage,
    SetSdpRequest, SetSdpResponse, StartSessionRequest, StopSessionRequest,
};

type ObserverStream =
    Pin<Box<dyn Stream<Item = Result<PeerConnectionObserverMessage, Status>> + Send>>;

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

impl From<libwebrtc::ice_candidate::ICECandidate> for webrtc::PeerConnectionObserverMessage {
    fn from(candidate: libwebrtc::ice_candidate::ICECandidate) -> Self {
        Self {
            event: Some(
                webrtc::peer_connection_observer_message::Event::IceCandidate(
                    webrtc::IceCandidate {
                        sdp: candidate.sdp(),
                        mid: candidate.sdp_mid(),
                        mline_index: candidate.sdp_mline_index(),
                    },
                ),
            ),
        }
    }
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

impl From<TransceiverDirection> for webrtc::TransceiverDirection {
    fn from(d: TransceiverDirection) -> Self {
        match d {
            TransceiverDirection::SendRecv => webrtc::TransceiverDirection::Sendrecv,
            TransceiverDirection::SendOnly => webrtc::TransceiverDirection::Sendonly,
            TransceiverDirection::RecvOnly => webrtc::TransceiverDirection::Recvonly,
            TransceiverDirection::Inactive => webrtc::TransceiverDirection::Inactive,
        }
    }
}

impl From<MediaType> for webrtc::MediaType {
    fn from(d: MediaType) -> Self {
        match d {
            MediaType::Audio => webrtc::MediaType::Audio,
            MediaType::Video => webrtc::MediaType::Video,
            MediaType::Data => webrtc::MediaType::Data,
            MediaType::Unsupported => webrtc::MediaType::Unsupported,
        }
    }
}

#[tonic::async_trait]
impl WebRtc for SharedState {
    type ObserverStream = ObserverStream;

    async fn create_session(
        &self,
        request: Request<CreateSessionRequest>,
    ) -> Result<Response<CreateSessionResponse>, Status> {
        let name = requester("create_session", request).name;
        let session = Session::new(name)?;
        let session_id = session.id.clone();
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
        let CreatePeerConnectionRequest { name, session_id } =
            requester("create_peer_connection", request);
        let peer_connection_id = nanoid::nanoid!();
        let pool = &get_session_attribute!(self, session_id.clone(), webrtc_pool);
        // create the peer connection
        let session = self.data.get_session(&session_id)?;
        let peer_connection =
            pool.create_peer_connection_manager(peer_connection_id.clone(), name)?;

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
        let pool = &session.value().webrtc_pool;

        pc.value()
            .add_track(pool, video_source, track_label)
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
        let pool = &session.value().webrtc_pool;
        let video_source = &session.value().video_source;
        pc.value()
            .add_transceiver(pool, video_source, track_label)
            .await?;
        let reply = Empty {};

        responder("add_transceiver", reply)
    }

    async fn observer(
        &self,
        request: tonic::Request<webrtc::ObserverRequest>,
    ) -> Result<tonic::Response<ObserverStream>, tonic::Status> {
        let request = requester("observer", request);
        let session_id = request.session_id;
        let peer_connection_id = request.peer_connection_id;
        let session = self.data.get_session(&session_id)?;
        let mut pc = session
            .value()
            .peer_connections
            .get_mut(&peer_connection_id)
            .ok_or_else(|| tonic::Status::new(tonic::Code::NotFound, "PeerConnection not found"))?;

        let mut ice_rx = pc.value_mut().ice_candidates_rx()?;
        let stream_out = stream! {
            loop {
                select! {
                    candidate = ice_rx.recv() => {
                        match candidate.ok_or_else(|| ServerError::InternalError("observer ice candidate erorr".into())) {
                            Ok(candidate) => {
                                let message = webrtc::PeerConnectionObserverMessage {
                                    event: Some(
                                        webrtc::peer_connection_observer_message::Event::IceCandidate(
                                            webrtc::IceCandidate {
                                                sdp: candidate.sdp(),
                                                mid: candidate.sdp_mid(),
                                                mline_index: candidate.sdp_mline_index(),
                                            },
                                        ),
                                    ),
                                };
                                yield Ok(message);
                            },
                            Err(e) => {
                                error!("observer ice candidate error: {}", e);
                            }
                        };
                    }
                }
            }
        };

        Ok(tonic::Response::new(Box::pin(stream_out)))
    }

    async fn get_transceivers(
        &self,
        request: tonic::Request<webrtc::GetTransceiversRequest>,
    ) -> Result<tonic::Response<webrtc::GetTransceiversResponse>, tonic::Status> {
        let request = requester("observer", request);
        let session_id = request.session_id;
        let peer_connection_id = request.peer_connection_id;
        let session = self.data.get_session(&session_id)?;
        let pc = session
            .value()
            .peer_connections
            .get(&peer_connection_id)
            .ok_or_else(|| tonic::Status::new(tonic::Code::NotFound, "PeerConnection not found"))?;
        let (video, audio) = pc.get_transceivers().await;
        let mut result = vec![];
        video.into_iter().for_each(|t| {
            let direction: webrtc::TransceiverDirection = t.direction().into();
            let media_type: webrtc::MediaType = t.media_type().into();
            let transceiver = webrtc::Transceiver {
                id: "".to_owned(),
                mid: t.mid(),
                direction: direction.into(),
                media_type: media_type.into(),
            };
            result.push(transceiver);
        });
        audio.into_iter().for_each(|t| {
            let direction: webrtc::TransceiverDirection = t.direction().into();
            let media_type: webrtc::MediaType = t.media_type().into();
            let transceiver = webrtc::Transceiver {
                id: "".to_owned(),
                mid: t.mid(),
                direction: direction.into(),
                media_type: media_type.into(),
            };
            result.push(transceiver);
        });
        let reply = webrtc::GetTransceiversResponse {
            transceivers: result,
        };
        responder("get_transceivers", reply)
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
