use std::sync::atomic::{AtomicUsize, Ordering};

use dashmap::DashMap;
use libwebrtc::{
    error::WebRTCError,
    factory::{Factory, FactoryConfig},
    passthrough_video_decoder_factory::PassthroughVideoDecoderFactory,
    peer_connection::PeerConnectionFactory,
    reactive_video_encoder::ReactiveVideoEncoderFactory,
    video_encoder_pool::VideoEncoderPool,
};

use crate::{error::Result, peer_connection::PeerConnectionManager};

pub(crate) struct WebRTCPoolItem {
    pub(crate) id: u32,
    // Hold reference to facctory for potential future use in api.
    #[allow(dead_code)]
    pub(crate) factory: Factory,
    pub(crate) peer_connection_factory: PeerConnectionFactory,
    pub(crate) count: AtomicUsize,
}

impl std::fmt::Debug for WebRTCPoolItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebRTCPoolItem")
            .field("id", &self.id)
            .field("count", &self.count)
            .finish()
    }
}

#[derive(Debug)]
pub(crate) struct WebRTCPool {
    #[allow(dead_code)]
    pub(crate) factory_count: usize,
    pub(crate) factory_list: DashMap<u32, WebRTCPoolItem>,
    #[allow(dead_code)]
    pub(crate) video_encoder_pool: VideoEncoderPool,
}

impl WebRTCPool {
    pub(crate) fn new(factory_count: usize) -> Result<Self> {
        let (video_encoder_pool, video_encoder_pool_tx) = VideoEncoderPool::create()?;
        let factory_list = DashMap::new();
        for id in 0u32..(factory_count as u32) {
            let factory = Factory::new();
            let reactive_video_encoder =
                ReactiveVideoEncoderFactory::create(video_encoder_pool_tx.clone())?;
            let peer_connection_factory = factory.create_factory_with_config(FactoryConfig {
                video_encoder_factory: Some(Box::new(reactive_video_encoder)),
                video_decoder_factory: Some(Box::new(PassthroughVideoDecoderFactory::new())),
                audio_encoder_factory: None,
            })?;
            let item = WebRTCPoolItem {
                id,
                factory,
                peer_connection_factory,
                count: AtomicUsize::new(0),
            };
            factory_list.insert(id, item);
        }
        Ok(Self {
            factory_count,
            factory_list,
            video_encoder_pool,
        })
    }

    pub(crate) fn create_peer_connection_manager(
        &self,
        id: String,
        name: String,
    ) -> Result<PeerConnectionManager> {
        let iter = self.factory_list.iter();

        let item = iter
            .min_by(|x, y| {
                let x_count = x.value().count.load(Ordering::Relaxed);
                let y_count = y.value().count.load(Ordering::Relaxed);
                x_count.cmp(&y_count)
            })
            .ok_or_else(|| WebRTCError::UnexpectedError("No peer connection factories".into()))?;

        let pool_id = item.key();
        item.value().count.fetch_add(1, Ordering::Relaxed);
        PeerConnectionManager::new(&item.value().peer_connection_factory, *pool_id, id, name)
    }
}
