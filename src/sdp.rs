use crate::offer_websocket::OfferWebSocketError;
use log::error;
use sdp::{
    common_description::{Address, Attribute, Bandwidth, ConnectionInformation},
    media_description::{MediaDescription, MediaName, RangedPort},
    session_description::{
        Origin, RepeatTime, SessionDescription, TimeDescription, TimeZone, Timing,
    },
};
use std::vec;

#[derive(Debug, PartialEq)]
pub(crate) enum ActiveMode {
    Active,
    Passive,
    ActivePassive,
}

#[derive(Debug)]
pub(crate) struct ProxyHandlerSDPConfig {
    pub(crate) remote_ice_username: String,
    pub(crate) remote_ice_password: String,
    pub(crate) fingerprint: String,
    pub(crate) active_mode: ActiveMode,
}

pub(crate) fn parse_sdp_config(
    sdp: &SessionDescription,
    fingerprint: String,
) -> Result<ProxyHandlerSDPConfig, OfferWebSocketError> {
    let media = &sdp.media_descriptions;
    let mut ice_username: Option<String> = None;
    let mut ice_password: Option<String> = None;
    let mut active_mode: Option<ActiveMode> = None;

    for attr in media {
        for k in &attr.attributes {
            match k.key.to_owned().as_str() {
                "ice-ufrag" => match k.value.to_owned() {
                    Some(v) => ice_username = Some(v),
                    _ => {}
                },
                "ice-pwd" => match k.value.to_owned() {
                    Some(v) => ice_password = Some(v),
                    _ => {}
                },
                "setup" => match k.value.to_owned() {
                    Some(v) => {
                        active_mode = Some(match v.as_str() {
                            "active" => ActiveMode::Active,
                            "passive" => ActiveMode::Passive,
                            "actpass" => ActiveMode::ActivePassive,
                            _ => {
                                error!("unknown a=setup field");
                                ActiveMode::ActivePassive
                            }
                        })
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    if ice_username.is_none() {
        return Err(OfferWebSocketError::InvalidSDP(String::from(
            "missing ice username",
        )));
    }

    if ice_password.is_none() {
        return Err(OfferWebSocketError::InvalidSDP(String::from(
            "missing ice username",
        )));
    }

    if active_mode.is_none() {
        return Err(OfferWebSocketError::InvalidSDP(String::from(
            "no active mode found",
        )));
    }

    Ok(ProxyHandlerSDPConfig {
        remote_ice_password: ice_password.unwrap(),
        remote_ice_username: ice_username.unwrap(),
        fingerprint,
        active_mode: active_mode.unwrap(),
    })
}

// We must craft an answer based on the original offer and accept all media and bandwidth.
pub(crate) async fn create_answer(
    offer_sdp: &SessionDescription,
    local_username: String,
    local_password: String,
    active_mode: &ActiveMode,
    fingerprint: &str,
) -> SessionDescription {
    let mut attributes: Vec<Attribute> = vec![];
    for attr in &offer_sdp.attributes {
        attributes.push(Attribute {
            key: attr.key.to_owned(),
            value: attr.value.to_owned(),
        });
    }

    let mut media_desc_vec: Vec<MediaDescription> = vec![];
    for media_desc in &offer_sdp.media_descriptions {
        let media_name = MediaName {
            media: media_desc.media_name.media.to_owned(),
            port: RangedPort {
                value: media_desc.media_name.port.value.to_owned(),
                range: media_desc.media_name.port.range.to_owned(),
            },
            protos: media_desc.media_name.protos.to_owned(),
            formats: media_desc.media_name.formats.to_owned(),
        };
        let media_title = media_desc.media_title.to_owned();
        let connection_information = match &offer_sdp.connection_information {
            Some(val) => {
                let address = match &val.address {
                    Some(addr) => Some(Address {
                        address: addr.address.to_owned(),
                        ttl: addr.ttl.to_owned(),
                        range: addr.range.to_owned(),
                    }),
                    None => None,
                };
                Some(ConnectionInformation {
                    network_type: val.network_type.to_owned(),
                    address: address,
                    address_type: val.address_type.to_owned(),
                })
            }
            None => None,
        };

        let mut bandwidth_vec: Vec<Bandwidth> = vec![];
        for bandwidth_attr in &media_desc.bandwidth {
            let experimental = bandwidth_attr.experimental.to_owned();
            let bandwidth_type = bandwidth_attr.bandwidth_type.to_owned();
            let bandwidth = bandwidth_attr.bandwidth.to_owned();
            bandwidth_vec.push(Bandwidth {
                experimental: experimental,
                bandwidth: bandwidth,
                bandwidth_type: bandwidth_type,
            });
        }

        let encryption_key = media_desc.encryption_key.clone();
        let mut attributes: Vec<Attribute> = vec![];

        for attr in &media_desc.attributes {
            match attr.key.as_str() {
                "sendonly" => {
                    let new_attr = Attribute {
                        key: "recvonly".to_owned(),
                        value: None,
                    };
                    attributes.push(new_attr);
                }
                "sendrecv" => {
                    let new_attr = Attribute {
                        key: "recvonly".to_owned(),
                        value: None,
                    };
                    attributes.push(new_attr);
                }
                "ice-ufrag" => {
                    let new_attr = Attribute {
                        key: attr.key.to_owned(),
                        value: Some(local_username.to_owned()),
                    };
                    attributes.push(new_attr);
                }
                "ice-pwd" => {
                    let new_attr = Attribute {
                        key: attr.key.to_owned(),
                        value: Some(local_password.to_owned()),
                    };
                    attributes.push(new_attr);
                }
                "fingerprint" => {
                    let new_attr = Attribute {
                        key: attr.key.to_owned(),
                        value: Some(format!("sha-256 {}", fingerprint).to_owned()),
                    };
                    attributes.push(new_attr);
                }
                "setup" => {
                    //   match cfg.
                    match active_mode {
                        ActiveMode::Active => {
                            let new_attr = Attribute {
                                key: attr.key.to_owned(),
                                value: Some("passive".to_owned()),
                            };
                            attributes.push(new_attr);
                        }
                        ActiveMode::ActivePassive => {
                            let new_attr = Attribute {
                                key: attr.key.to_owned(),
                                value: Some("active".to_owned()),
                            };
                            attributes.push(new_attr);
                        }
                        ActiveMode::Passive => {
                            let new_attr = Attribute {
                                key: attr.key.to_owned(),
                                value: Some("active".to_owned()),
                            };
                            attributes.push(new_attr);
                        }
                    }
                }
                _ => {
                    let new_attr = Attribute {
                        key: attr.key.to_owned(),
                        value: attr.value.to_owned(),
                    };
                    attributes.push(new_attr);
                }
            }
        }

        let new_media_desc = MediaDescription {
            media_name: media_name,
            media_title: media_title,
            connection_information: connection_information,
            bandwidth: bandwidth_vec,
            encryption_key: encryption_key,
            attributes: attributes,
        };

        media_desc_vec.push(new_media_desc);
    }

    let origin = Origin {
        username: offer_sdp.origin.username.to_owned(),
        session_id: offer_sdp.origin.session_id.to_owned(),
        session_version: offer_sdp.origin.session_version,
        network_type: offer_sdp.origin.network_type.to_owned(),
        address_type: offer_sdp.origin.address_type.to_owned(),
        unicast_address: offer_sdp.origin.unicast_address.to_owned(),
    };

    let connection_information = match &offer_sdp.connection_information {
        Some(val) => {
            let address = match &val.address {
                Some(addr) => Some(Address {
                    address: addr.address.to_owned(),
                    ttl: addr.ttl.to_owned(),
                    range: addr.range.to_owned(),
                }),
                None => None,
            };
            Some(ConnectionInformation {
                network_type: val.network_type.to_owned(),
                address: address,
                address_type: val.address_type.to_owned(),
            })
        }
        None => None,
    };

    let mut bandwidth_vec: Vec<Bandwidth> = vec![];
    for bandwidth_attr in &offer_sdp.bandwidth {
        let experimental = bandwidth_attr.experimental.to_owned();
        let bandwidth_type = bandwidth_attr.bandwidth_type.to_owned();
        let bandwidth = bandwidth_attr.bandwidth.to_owned();
        bandwidth_vec.push(Bandwidth {
            experimental: experimental,
            bandwidth: bandwidth,
            bandwidth_type: bandwidth_type,
        });
    }

    let mut time_desc_vec: Vec<TimeDescription> = vec![];
    let timezone_vec: Vec<TimeZone> = vec![];

    for time_desc_attr in &offer_sdp.time_descriptions {
        let timing = Timing {
            start_time: time_desc_attr.timing.start_time.to_owned(),
            stop_time: time_desc_attr.timing.stop_time.to_owned(),
        };

        let mut repeat_times: Vec<RepeatTime> = vec![];

        for repeat_time_attr in &time_desc_attr.repeat_times {
            let repeat = RepeatTime {
                interval: repeat_time_attr.interval.to_owned(),
                duration: repeat_time_attr.interval.to_owned(),
                offsets: repeat_time_attr.offsets.to_vec(),
            };
            repeat_times.push(repeat);
        }

        let time_desc = TimeDescription {
            timing: timing,
            repeat_times: repeat_times,
        };

        time_desc_vec.push(time_desc);
    }

    SessionDescription {
        version: offer_sdp.version.to_owned(),
        origin: origin,
        session_name: offer_sdp.session_name.to_owned(),
        session_information: offer_sdp.session_information.to_owned(),
        uri: offer_sdp.uri.to_owned(),
        email_address: offer_sdp.email_address.to_owned(),
        phone_number: offer_sdp.phone_number.to_owned(),
        connection_information: connection_information,
        bandwidth: bandwidth_vec,
        time_descriptions: time_desc_vec,
        time_zones: timezone_vec,
        encryption_key: offer_sdp.encryption_key.to_owned(),
        attributes: attributes,
        media_descriptions: media_desc_vec,
    }
}
