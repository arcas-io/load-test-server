use ::srtp as srtp_protection;
use futures_util::{stream::SplitSink, SinkExt, StreamExt};
use log::{error, info};
use openssl::{
    srtp::{self, SrtpProfileId},
    ssl::{Ssl, SslConnector, SslMethod, SslRef},
};
use sdp::session_description::SessionDescription;
use serde::{Deserialize, Serialize};
use srtp_protection::sys::{
    srtp_install_log_handler, srtp_profile_get_master_key_length,
    srtp_profile_get_master_salt_length, srtp_profile_t, SRTP_MAX_KEY_LEN,
};
use std::io::{Cursor, Read};
use std::pin::Pin;
use std::sync::Arc;
use thiserror::Error;
use tokio::{
    io::AsyncReadExt,
    sync::{
        mpsc::{self, Receiver},
        mpsc::{channel, Sender},
        oneshot, Mutex, MutexGuard,
    },
};
use tokio_openssl::SslStream;
use warp::ws::{Message, WebSocket};
use webrtc_dtls::conn::DTLSConn;
use webrtc_ice::{
    agent::agent_config::AgentConfig, candidate::Candidate, mdns::MulticastDnsMode,
    network_type::NetworkType, url::Url,
};
use webrtc_ice::{agent::Agent, state::ConnectionState};
use webrtc_util::Conn;

use crate::dtls::{ssl_client, ssl_server};
use crate::endpoint_read_write::EndpointReadWrite;
use crate::mux::{self, endpoint::Endpoint};
use crate::mux::{mux_func::match_range, Config as MuxConfig, Mux};
use crate::sdp::{create_answer, parse_sdp_config, ActiveMode, ProxyHandlerSDPConfig};
use crate::utils::{log_error, CERTIFICATE};
use crate::{crypto::fingerprint, mux::mux_func::match_srtp};

const ANSWER_KIND: &'static str = "answer";
const CANDIDATE_KIND: &'static str = "candidate";
const CANDIDATE_END_KIND: &'static str = "candidate_end";
const RECEIVE_MTU: usize = 1460;

#[derive(Debug, Serialize)]
enum WebSocketOutput {
    Text(String),
    Error,
}

#[derive(Error, Debug)]
pub enum OfferWebSocketError {
    #[error("Parse error")]
    ParseFailed(serde_json::Error),
    #[error("Parse error")]
    SerializeFailed(serde_json::Error),
    #[error("Serializing response failed")]
    Unhandled,
    #[error("unknown error hit")]
    UnknownError(Box<dyn std::error::Error>),
    #[error("Invalid websocket message type")]
    InvalidMessage,
    #[error("Invalid SDP")]
    InvalidSDP(String),
    #[error("Invalid ice agent configuration")]
    InvalidAgentConfig,
    #[error("Error during gathering")]
    GatheringError,
    #[error("WebSocket write error")]
    WebSocketWriteError,
    #[error("Internal error")]
    InternalError(String),
    #[error("No SRTP protection profile")]
    NoProtectionProfile,
    #[error("Invalid protection profile")]
    InvalidProtectionProfile,
}

enum Response<'a> {
    Candidate(WSResponseCandidate<'a>),
    Answer(WSResponseAnswer<'a>),
}

enum ProxyAgentState {
    New,
    Gathering,
    ICEReady,
}

enum ProxyMessageState {
    Offer,
    Candidate,
    CandidatesEnd,
}

#[derive(Deserialize, Debug)]
struct WSRequestOffer {
    sdp: String,
}

#[derive(Deserialize, Serialize, Debug)]
struct WSResponseAnswer<'a> {
    kind: &'a str,
    sdp: String,
}

#[derive(Deserialize, Serialize, Debug)]
struct WSRequestCandidate {
    candidate: String,
}

#[derive(Deserialize, Serialize, Debug)]
struct WSResponseCandidate<'a> {
    kind: &'a str,
    candidate: String,
}

struct ProtectionProfile {
    kind: SrtpProfileId,
    client_key: Vec<u8>,
    server_key: Vec<u8>,
}

struct ProxyHandler {
    writer: Arc<Mutex<SplitSink<WebSocket, Message>>>,
    message_state: Arc<ProxyMessageState>,
    offer: Option<Arc<SessionDescription>>,
    sdp_config: Option<Arc<ProxyHandlerSDPConfig>>,
    ice_state: Arc<ProxyAgentState>,
    ice_agent: Arc<Option<Arc<Mutex<Agent>>>>,
    mux: Option<Arc<mux::Mux>>,
    protection_profile: Option<Arc<ProtectionProfile>>,
}

impl ProxyHandler {
    fn new(writer: SplitSink<WebSocket, Message>) -> ProxyHandler {
        ProxyHandler {
            writer: Arc::new(Mutex::new(writer)),
            offer: None,
            message_state: Arc::new(ProxyMessageState::Offer),
            sdp_config: None,
            ice_state: Arc::new(ProxyAgentState::New),
            ice_agent: Arc::new(None),
            mux: None,
            protection_profile: None,
        }
    }

    async fn send<'a>(&self, response: Response<'a>) -> Result<(), OfferWebSocketError> {
        let res = match response {
            Response::Answer(answer) => serde_json::to_string(&answer),
            Response::Candidate(candidate) => serde_json::to_string(&candidate),
        }
        .map_err(|err| {
            error!("serialize error : {:?}", err);
            OfferWebSocketError::SerializeFailed(err)
        })?;

        match self.writer.lock().await.send(Message::text(res)).await {
            Err(err) => {
                error!("error sending : {:?}", err);
                return Err(OfferWebSocketError::WebSocketWriteError);
            }
            _ => {}
        };

        Ok(())
    }

    async fn start_handshake(
        &mut self,
        offer: SessionDescription,
    ) -> Result<(), OfferWebSocketError> {
        let stun_url = Url {
            scheme: webrtc_ice::url::SchemeType::Stun,
            host: "stun.l.google.com".to_owned(),
            port: 19302,
            proto: webrtc_ice::url::ProtoType::Udp,
            username: "".to_owned(),
            password: "".to_owned(),
        };

        let agent = Agent::new(AgentConfig {
            urls: vec![stun_url],
            network_types: vec![NetworkType::Udp4],
            multicast_dns_mode: MulticastDnsMode::Disabled,
            ..Default::default()
        })
        .await
        .map_err(|err| OfferWebSocketError::InvalidAgentConfig)?;

        let fingerprint = fingerprint(&(*CERTIFICATE).0)?;

        self.ice_agent = Arc::new(Some(Arc::new(Mutex::new(agent))));
        self.sdp_config = Some(Arc::new(parse_sdp_config(&offer, fingerprint)?));
        self.offer = Some(Arc::new(offer));
        self.ice_state = Arc::new(ProxyAgentState::Gathering);

        let agent_unwrap = self.ice_agent.as_deref().unwrap();
        let agent = agent_unwrap.lock().await;

        agent
            .on_connection_state_change(Box::new(|state: ConnectionState| {
                Box::pin(async move {
                    info!("ice connection state change {:?}", state);
                })
            }))
            .await;

        let candidates: Arc<Mutex<Vec<Box<Arc<dyn Candidate + Send + Sync>>>>> =
            Arc::new(Mutex::new(Vec::new()));

        let callback_candidates = candidates.clone();
        let (candidates_ready_sender, mut candidates_ready) = channel::<()>(1);

        agent
            .on_candidate(Box::new(
                move |candidate: Option<Arc<dyn Candidate + Send + Sync>>| {
                    let candidates = callback_candidates.clone();
                    let tx = candidates_ready_sender.clone();
                    Box::pin(async move {
                        match candidate {
                            Some(candidate) => {
                                candidates.lock().await.push(Box::new(candidate));
                            }
                            None => match tx.send(()).await {
                                Err(_) => {
                                    error!("error sending ready for candidate end");
                                }
                                _ => {}
                            },
                        };
                    })
                },
            ))
            .await;

        agent.gather_candidates().await.map_err(|err| {
            error!("Gathering error {:?}", err);
            OfferWebSocketError::GatheringError
        })?;

        // wait for all candidates to be gathered
        let _ = candidates_ready.recv().await;
        // this is the full candidate list
        let candidate_list = candidates.lock().await;

        for candidate in &*candidate_list {
            let response = WSResponseCandidate {
                kind: CANDIDATE_KIND,
                candidate: format!("candidate:{}", candidate.marshal()),
            };
            self.send(Response::Candidate(response)).await?;

            info!("sent candidate: {:?}", candidate.marshal());
        }

        info!("handshake complete");

        Ok(())
    }

    async fn setup_ice_client(&self) -> Result<Arc<impl Conn + Sync + Send>, OfferWebSocketError> {
        info!("setting up as ice client");
        let cfg = &self.get_sdp_config()?;
        let agent_mutex = self.get_agent()?;
        let agent = agent_mutex.lock().await;

        let (cancel_tx, cancel_rx) = mpsc::channel::<()>(1);
        let conn = agent
            .dial(
                cancel_rx,
                cfg.remote_ice_username.to_owned(),
                cfg.remote_ice_password.to_owned(),
            )
            .await
            .map_err(|e| log_error("IceClientSetupError", e))?;

        info!("success getting connection");

        Ok(conn)
    }

    async fn setup_ice_server(&self) -> Result<Arc<impl Conn + Sync + Send>, OfferWebSocketError> {
        info!("setting up as ice server");
        let cfg = self.get_sdp_config()?;
        let agent_mutex = self.get_agent()?;
        let agent = agent_mutex.lock().await;

        let (cancel_tx, cancel_rx) = mpsc::channel::<()>(1);
        info!(
            "ice user: {} pass: {}",
            cfg.remote_ice_username, cfg.remote_ice_password
        );
        agent
            .set_remote_credentials(
                cfg.remote_ice_username.to_owned(),
                cfg.remote_ice_password.to_owned(),
            )
            .await
            .map_err(|e| log_error("IceRemoteCredsError", e))?;
        let conn = agent
            .accept(
                cancel_rx,
                cfg.remote_ice_username.to_owned(),
                cfg.remote_ice_password.to_owned(),
            )
            .await
            .map_err(|e| log_error("IceConnectError", e))?;

        Ok(conn)
    }

    fn get_sdp_config(&self) -> Result<Arc<ProxyHandlerSDPConfig>, OfferWebSocketError> {
        match &self.sdp_config {
            None => Err(log_error("NoSdpConfig", "")),
            Some(config) => Ok(config.clone()),
        }
    }

    fn get_agent(&self) -> Result<Arc<&Mutex<Agent>>, OfferWebSocketError> {
        self.ice_agent
            .as_deref()
            .ok_or(log_error("NoIceAgent", ""))
            .and_then(|agent| Ok(Arc::new(agent)))
    }

    fn get_protection_profile(&self) -> Result<Arc<ProtectionProfile>, OfferWebSocketError> {
        match &self.protection_profile {
            Some(profile) => Ok(profile.clone()),
            None => Err(OfferWebSocketError::InternalError(
                "failed to get protection profile".to_owned(),
            )),
        }
    }

    // We must craft an answer based on the original offer and accept all media and bandwidth.
    async fn create_answer(&self) -> Result<SessionDescription, OfferWebSocketError> {
        let agent = self.get_agent()?;
        let cfg = self.get_sdp_config()?;
        let (local_username, local_password) =
            agent.lock().await.get_local_user_credentials().await;
        let offer_sdp = self
            .offer
            .as_ref()
            .ok_or(log_error("CreateAnswerError", ""))?;

        let answer_sdp = create_answer(
            offer_sdp,
            local_username,
            local_password,
            &cfg.active_mode,
            &cfg.fingerprint,
        )
        .await;

        Ok(answer_sdp)
    }

    async fn handle_end_of_candidates(&mut self) -> Result<(), OfferWebSocketError> {
        let cfg = self.get_sdp_config()?;

        // send answer?
        let answer_sdp = self.create_answer().await?;

        info!("S {:?}", answer_sdp.marshal());

        let answer_response = WSResponseAnswer {
            kind: ANSWER_KIND,
            sdp: answer_sdp.marshal(),
        };

        self.send(Response::Answer(answer_response)).await?;
        let is_client = cfg.active_mode == ActiveMode::Active;
        // TODO: add strum and uncomment below
        // info!("active mode = {}", ActiveMode::Active);

        // impl Conn all return distinct types so we need some copy/pasta here.
        let srtp_endpoint = match is_client {
            true => {
                let conn = self.setup_ice_client().await?;
                let (dtls_endpoint, srtp_endpoint) = self.add_mux(conn.clone()).await?;
                self.dtls_connect(is_client, dtls_endpoint).await?;
                srtp_endpoint
            }
            false => {
                let conn = self.setup_ice_server().await?;
                let (dtls_endpoint, srtp_endpoint) = self.add_mux(conn.clone()).await?;
                self.dtls_connect(is_client, dtls_endpoint).await?;
                srtp_endpoint
            }
        };

        self.read_srtp(srtp_endpoint).await?;
        Ok(())
    }

    async fn add_mux(
        &mut self,
        conn: Arc<impl Conn + Send + Sync + 'static>,
    ) -> Result<(Arc<Endpoint>, Arc<Endpoint>), OfferWebSocketError> {
        let mux_config = MuxConfig {
            conn: conn,
            buffer_size: RECEIVE_MTU,
        };
        let mux = Mux::new(mux_config);
        let dtls_endpoint = mux.new_endpoint(match_range(20, 63)).await;
        let srtp_endpoint = mux.new_endpoint(match_range(128, 191)).await;
        self.mux = Some(Arc::new(mux));

        Ok((dtls_endpoint, srtp_endpoint))
    }

    async fn dtls_connect(
        &mut self,
        is_client: bool,
        dtls_endpoint: Arc<Endpoint>,
    ) -> Result<(), OfferWebSocketError> {
        info!("Begin DTSL handshake");
        let dtls_stream_wrapper = EndpointReadWrite::new(dtls_endpoint);
        let ssl_dtls = ssl_client(SslMethod::dtls()).unwrap();
        let mut dtls_ssl_stream = SslStream::new(ssl_dtls, dtls_stream_wrapper)
            .map_err(|e| log_error("DtlsStreamError", e))
            .unwrap();

        let stream_result = if is_client {
            Pin::new(&mut dtls_ssl_stream).accept().await
        } else {
            Pin::new(&mut dtls_ssl_stream).connect().await
        };

        stream_result.map_err(|e| log_error("DtlsStreamConnectError", e))?;
        info!("DTLS handshake complete");
        self.extract_srtp_info(dtls_ssl_stream.ssl()).await?;
        Ok(())
    }

    fn get_crypto_policy(&self) -> Result<srtp_protection::CryptoPolicy, OfferWebSocketError> {
        let protection_profile = self.get_protection_profile()?;
        let crypto = match protection_profile.kind {
            SrtpProfileId::SRTP_AES128_CM_SHA1_80 => {
                info!("HMAC !");
                srtp_protection::CryptoPolicy::aes_cm_128_hmac_sha1_80()
            }
            SrtpProfileId::SRTP_AEAD_AES_128_GCM => {
                srtp_protection::CryptoPolicy::aes_gcm_128_8_auth()
            }
            _ => {
                return Err(OfferWebSocketError::InvalidProtectionProfile);
            }
        };

        Ok(crypto)
    }

    async fn read_srtp(&mut self, srtp_endpoint: Arc<Endpoint>) -> Result<(), OfferWebSocketError> {
        let protection_profile = self.get_protection_profile()?;
        let mut session =
            srtp_protection::Session::with_inbound_template(srtp_protection::StreamPolicy {
                key: &protection_profile.server_key.as_slice(),
                // protection_profile,
                rtp: self.get_crypto_policy()?,
                rtcp: self.get_crypto_policy()?,
                ..Default::default()
            })
            .map_err(|err| log_error("srtp protection setup", err))?;

        loop {
            let mut buf = [0; 1400];
            let bytes_read = match srtp_endpoint.recv(&mut buf).await {
                Ok(bytes_read) => bytes_read,
                Err(err) => return Err(log_error("SRTPRead", err)),
            };

            let is_rtp = match_srtp(&buf);
            info!(
                "read {:?} bytes off the wire for SRTP (rtp = {:?})",
                bytes_read, is_rtp
            );
            let vec = &mut buf[0..bytes_read].to_vec();
            info!("before: {:?}", vec.len());

            match is_rtp {
                true => {
                    session
                        .unprotect(vec)
                        .map_err(|err| log_error("srtp unprotect", err))?;
                }
                false => {
                    session
                        .unprotect_rtcp(vec)
                        .map_err(|err| log_error("srtcp unprotect", err))?;
                }
            }
            info!("acter: {:?}", vec.len());
        }

        Ok(())
    }

    async fn extract_srtp_info(&mut self, ssl: &SslRef) -> Result<(), OfferWebSocketError> {
        let profile = match ssl.selected_srtp_profile() {
            Some(profile) => profile,
            None => return Err(OfferWebSocketError::NoProtectionProfile),
        };

        // https://github.com/pion/srtp/blob/82008b58b1e7be7a0cb834270caafacc7ba53509/protection_profile.go

        let (profile, master_key_len, master_salt_len) = match profile.id() {
            SrtpProfileId::SRTP_AES128_CM_SHA1_80 => {
                info!("using aes");
                // 16 for key and 14 for the salt * 2
                (SrtpProfileId::SRTP_AES128_CM_SHA1_80, 16, 14)
            }
            SrtpProfileId::SRTP_AEAD_AES_128_GCM => {
                info!("using aead");
                // 16 for key and 12 for the salt * 2
                (SrtpProfileId::SRTP_AEAD_AES_128_GCM, 16, 12)
            }
            _ => return Err(OfferWebSocketError::InvalidProtectionProfile),
        };

        // https://github.com/HyeonuPark/srtp/blob/e853208c8dda77daef7d3a58c4ead01b53f062ed/src/openssl.rs#L106
        let mut buf = [0; SRTP_MAX_KEY_LEN as usize * 2];
        let master_len = master_key_len + master_salt_len;

        ssl.export_keying_material(&mut buf, "EXTRACTOR-dtls_srtp", None)
            .map_err(|e| log_error("DTLSExportKey", e))?;

        let rot_start = master_key_len;
        let rot_end = rot_start + master_len;

        buf[rot_start..rot_end].rotate_left(master_key_len);

        let client_key: &[u8] = &buf[..master_len];
        let server_key: &[u8] = &buf[master_len..(2 * master_len)];

        self.protection_profile = Some(Arc::new(ProtectionProfile {
            kind: profile,
            server_key: server_key.to_vec(),
            client_key: client_key.to_vec(),
        }));

        Ok(())
    }

    pub async fn handle_candidate(
        &mut self,
        request: WSRequestCandidate,
    ) -> Result<(), OfferWebSocketError> {
        let agent = self
            .ice_agent
            .as_deref()
            .ok_or(log_error("WsMissingIceAgentError", ""))?;

        if request.candidate.len() == 0 {
            self.message_state = Arc::new(ProxyMessageState::CandidatesEnd);
            self.handle_end_of_candidates().await?;
            return Ok(());
        }

        match agent
            .lock()
            .await
            .unmarshal_remote_candidate(request.candidate)
            .await
        {
            Err(err) => {
                error!("failed to add candidate: {:?}", err);
            }
            _ => {
                info!("successfully added candidate")
            }
        }

        Ok(())
    }

    pub async fn handle_offer(
        &mut self,
        request: WSRequestOffer,
    ) -> Result<(), OfferWebSocketError> {
        let mut cursor = Cursor::new(request.sdp.as_bytes());
        let offer = SessionDescription::unmarshal(&mut cursor)
            .map_err(|err| OfferWebSocketError::InvalidSDP(String::from("failed to parse")))?;

        self.message_state = Arc::new(ProxyMessageState::Candidate);
        self.start_handshake(offer).await?;

        Ok(())
    }

    pub async fn handle_message(&mut self, msg: Message) -> Result<(), OfferWebSocketError> {
        match *self.message_state {
            ProxyMessageState::Offer => {
                info!("[ws] offer received");
                let request = serde_json::from_slice::<WSRequestOffer>(msg.as_bytes())
                    .map_err(|err| OfferWebSocketError::ParseFailed(err))?;
                self.handle_offer(request).await?;
            }
            ProxyMessageState::Candidate => {
                info!("[ws] candidate received");
                let request = serde_json::from_slice::<WSRequestCandidate>(msg.as_bytes())
                    .map_err(|err| OfferWebSocketError::ParseFailed(err))?;
                self.handle_candidate(request).await?;
            }
            _ => {}
        };

        Ok(())
    }

    async fn terminate(&mut self) -> anyhow::Result<()> {
        Ok(self.writer.lock().await.send(Message::close()).await?)
    }
}

pub async fn handle_offer_websocket(websocket: WebSocket) {
    let (write, mut read) = websocket.split();
    let handle_mutex = Arc::new(Mutex::new(ProxyHandler::new(write)));

    while let Some(result) = read.next().await {
        let process_result = match result {
            Ok(message) => {
                let mut handle = handle_mutex.lock().await;
                handle.handle_message(message).await
            }
            Err(err) => Err(log_error("WsMessageReadError", err)),
        };

        match process_result {
            Err(err) => Err(log_error("WsProcessMessageError", err)),
            _ => Ok(()),
        };
    }
}
