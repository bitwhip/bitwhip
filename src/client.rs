use bytes::Bytes;
use local_ip_address::list_afinet_netifas;
use reqwest::header::{HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use serde::Deserialize;
use std::{
    error::Error,
    io::ErrorKind,
    net::{IpAddr, SocketAddr, SocketAddrV4},
    str::FromStr,
    time::{Duration, Instant},
};
use str0m::{
    change::{SdpAnswer, SdpOffer},
    format::Codec,
    media::{Direction as RtcDirection, MediaData, MediaKind, MediaTime, Mid},
    net::{Protocol, Receive},
    Candidate, Event, IceConnectionState, Input, Output, Rtc,
};
use tokio::net::UdpSocket;
use tracing::{debug, error, info, trace, warn};

#[derive(Debug, Deserialize)]
pub struct WhipClaims {
    pub whip_url: String,
    pub jti: String,
}

#[derive(Debug)]
pub enum WebrtcEvent {
    Continue,
    Media(MediaData),
    Disconnected,
}

#[derive(Debug)]
pub enum WebrtcError {
    ServerError(Box<dyn Error + Send + Sync>),
    SdpError,
    WebrtcError(Box<dyn Error + Send + Sync>),
    NetworkError(Box<dyn Error + Send + Sync>),
    SendError(String),
    NoCandidates,
}

pub struct Client {
    rtc: Rtc,
    socket: UdpSocket,
    local_socket_addr: SocketAddr,
    buf: [u8; 1500],
    video_mid: Option<Mid>,
    _audio_mid: Option<Mid>,
}

impl Client {
    pub async fn new() -> Result<Self, WebrtcError> {
        let socket = UdpSocket::bind("0.0.0.0:0".parse::<SocketAddrV4>().unwrap())
            .await
            .expect("Should bind udp socket");

        let mut rtc = Rtc::builder()
            .clear_codecs()
            .enable_h264(true)
            .set_stats_interval(Some(Duration::from_secs(2)))
            .set_reordering_size_video(1)
            .set_reordering_size_audio(1)
            .build();

        info!("local socket address: {:?}", socket.local_addr());

        // Discover host candidates
        let mut local_socket_addr = None;
        if let Ok(network_interfaces) = list_afinet_netifas() {
            for (name, ip) in network_interfaces {
                info!("iface: {} / {:?}", name, ip);
                match ip {
                    IpAddr::V4(ip4) => {
                        if !ip4.is_loopback() && !ip4.is_link_local() {
                            let socket_addr =
                                SocketAddr::new(ip, socket.local_addr().unwrap().port());
                            if socket_addr.to_string().starts_with("192") {
                                local_socket_addr = Some(socket_addr.clone());
                            }
                            rtc.add_local_candidate(
                                Candidate::host(socket_addr, str0m::net::Protocol::Udp)
                                    .expect("Failed to create local candidate"),
                            );
                        }
                    }
                    IpAddr::V6(_ip6) => {}
                }
            }
        } else {
            return Err(WebrtcError::NoCandidates);
        }

        let Some(local_socket_addr) = local_socket_addr else {
            return Err(WebrtcError::NoCandidates);
        };

        Ok(Self {
            socket,
            local_socket_addr,
            rtc,
            buf: [0; 1500],
            video_mid: None,
            _audio_mid: None,
        })
    }

    pub async fn send_whip_request(
        &mut self,
        url: &str,
        token: &Option<String>,
        direction: RtcDirection,
    ) -> Result<(), WebrtcError> {
        // Add receive tracks and generate an offer
        let mut change = self.rtc.sdp_api();
        self.video_mid = Some(change.add_media(
            MediaKind::Video,
            direction,
            Some("video_0".to_string()),
            Some("video_0".to_string()),
        ));

        let (offer, pending) = change.apply().ok_or(WebrtcError::SdpError)?;

        let offer_str = offer.to_sdp_string();
        info!("offer: {}", offer_str);
        info!("token: {:?}", token);
        info!("url: {}", url);

        let mut headers = reqwest::header::HeaderMap::new();

        if let Some(token) = &token {
            let authoriation_value = HeaderValue::from_str(&format!("Bearer {}", token))
                .map_err(|e| WebrtcError::ServerError(e.into()))?;
            headers.append(AUTHORIZATION, authoriation_value);
        }

        headers.append(
            CONTENT_TYPE,
            HeaderValue::from_str("application/sdp").unwrap(),
        );
        headers.append(ACCEPT, HeaderValue::from_str("application/sdp").unwrap());
        headers.append(USER_AGENT, HeaderValue::from_str("bitwhip").unwrap());

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| WebrtcError::ServerError(e.into()))?;

        let mut next_url =
            reqwest::Url::from_str(&url).map_err(|e| WebrtcError::ServerError(e.into()))?;
        let res = loop {
            let response = client
                .post(next_url.clone())
                .body(offer_str.clone())
                .send()
                .await
                .map_err(|e| WebrtcError::ServerError(e.into()))?;
            if response.status().is_redirection() {
                if let Some(location) = response
                    .headers()
                    .get(reqwest::header::HeaderName::from_static("location"))
                {
                    next_url = reqwest::Url::from_str(
                        location
                            .to_str()
                            .map_err(|e| WebrtcError::ServerError(e.into()))?,
                    )
                    .map_err(|e| WebrtcError::ServerError(e.into()))?;
                    info!("Redirect! Next URL: {:?}", next_url);
                    continue;
                }
            } else {
                break response;
            }
        };

        // get answer sdp from body
        let http_code = res.status();
        info!("status: {}", http_code);
        if http_code != reqwest::StatusCode::CREATED {
            return Err(WebrtcError::ServerError(
                format!("POST failed with status: {}", http_code).into(),
            ));
        }

        info!("headers: {:?}", res.headers());
        let answer = res
            .text()
            .await
            .map_err(|e| WebrtcError::ServerError(e.into()))?;

        self.rtc
            .sdp_api()
            .accept_answer(
                pending,
                SdpAnswer::from_sdp_string(&answer).map_err(|_| WebrtcError::SdpError)?,
            )
            .map_err(|_| WebrtcError::SdpError)?;

        Ok(())
    }

    pub fn accept_whip_request(&mut self, offer: String) -> Result<String, WebrtcError> {
        let offer = SdpOffer::from_sdp_string(&offer).map_err(|_| WebrtcError::SdpError)?;
        if let Ok(answer) = self.rtc.sdp_api().accept_offer(offer) {
            return Ok(answer.to_sdp_string());
        }

        return Err(WebrtcError::SdpError);
    }

    pub async fn recv<'a>(&mut self) -> Result<WebrtcEvent, WebrtcError> {
        trace!("recv poll_output()");
        let timeout = match self
            .rtc
            .poll_output()
            .map_err(|e| WebrtcError::WebrtcError(e.into()))?
        {
            Output::Event(event) => match event {
                Event::Connected => {
                    info!("connected");
                    return Ok(WebrtcEvent::Continue);
                }
                Event::IceConnectionStateChange(state) => {
                    info!("ice connection state change: {:?}", state);
                    match state {
                        IceConnectionState::Disconnected => return Ok(WebrtcEvent::Disconnected),
                        _ => return Ok(WebrtcEvent::Continue),
                    }
                }
                Event::MediaIngressStats(stats) => {
                    info!("egress stats: {:?}", stats);
                    return Ok(WebrtcEvent::Continue);
                }
                Event::MediaEgressStats(stats) => {
                    info!("egress stats: {:?}", stats);
                    return Ok(WebrtcEvent::Continue);
                }
                Event::PeerStats(stats) => {
                    info!("stats: {:?}", stats);
                    return Ok(WebrtcEvent::Continue);
                }
                Event::MediaData(media) => {
                    return Ok(WebrtcEvent::Media(media));
                }
                Event::MediaAdded(media) => {
                    info!("Media Added: {:?}", media);
                    info!("Codec Config: {:?}", self.rtc.codec_config());
                    return Ok(WebrtcEvent::Continue);
                }
                _ => {
                    return Ok(WebrtcEvent::Continue);
                }
            },
            Output::Timeout(timeout) => timeout,
            Output::Transmit(send) => {
                // Apply random packet loss to outbound traffic
                if let Err(e) = self.socket.send_to(&send.contents, send.destination).await {
                    debug!(
                        "sending to {} => {}, len {} error {:?}",
                        send.source,
                        send.destination,
                        send.contents.len(),
                        e
                    );
                };
                return Ok(WebrtcEvent::Continue);
            }
        };

        let duration = timeout - Instant::now();
        if duration.is_zero() {
            // Drive time forwards in rtc straight away.
            return match self.rtc.handle_input(Input::Timeout(Instant::now())) {
                Ok(_) => Ok(WebrtcEvent::Continue),
                Err(e) => {
                    error!("error handle input rtc: {:?}", e);
                    Ok(WebrtcEvent::Continue)
                }
            };
        }

        // Maximum delay is 1ms
        let duration = duration.min(Duration::from_millis(1));

        let input = match tokio::time::timeout(duration, self.socket.recv_from(&mut self.buf)).await
        {
            Ok(Ok((n, source))) => {
                // UDP data received.
                info!(
                    "received from {} => {}, len {}",
                    source,
                    SocketAddr::new(
                        self.local_socket_addr.ip(),
                        self.socket.local_addr().unwrap().port(),
                    ),
                    n
                );
                Input::Receive(
                    Instant::now(),
                    Receive {
                        proto: Protocol::Udp,
                        source,
                        destination: SocketAddr::new(
                            self.local_socket_addr.ip(),
                            self.socket.local_addr().unwrap().port(),
                        ),
                        contents: (&self.buf[..n]).try_into().expect("should webrtc"),
                    },
                )
            }
            Ok(Err(e)) => match e.kind() {
                ErrorKind::ConnectionReset => return Ok(WebrtcEvent::Continue),
                _ => {
                    error!("[TransportWebrtc] network error {:?}", e);
                    return Err(WebrtcError::NetworkError(e.into()));
                }
            },
            Err(_e) => {
                // Expected error for set_read_timeout().
                // One for windows, one for the rest.
                Input::Timeout(Instant::now())
            }
        };

        // Input is either a Timeout or Receive of data. Both drive the state forward.
        self.rtc
            .handle_input(input)
            .map_err(|e| WebrtcError::WebrtcError(e.into()))?;
        return Ok(WebrtcEvent::Continue);
    }

    pub fn send_video(&mut self, frame_data: Bytes, pts: Duration) -> Result<(), WebrtcError> {
        if let Some(mid) = self.video_mid {
            // TODO = maybe look this up once?
            let params = &self
                .rtc
                .codec_config()
                .find(|p| {
                    debug!("payload: {:?}", p);
                    p.spec().codec == Codec::H264
                        && p.spec().format.profile_level_id.unwrap_or(0) == 4382751
                })
                .cloned()
                .unwrap();
            if let Some(writer) = self.rtc.writer(mid) {
                let freq = params.spec().clock_rate;
                let media_time: MediaTime = pts.into();
                writer
                    .write(
                        params.pt(),
                        Instant::now(),
                        media_time.rebase(freq),
                        frame_data,
                    )
                    .map_err(|e| WebrtcError::SendError(e.to_string()))?;
            }
        } else {
            warn!("trying to send video without mid");
        }
        Ok(())
    }
}
