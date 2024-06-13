use std::net::{IpAddr, SocketAddrV4};
use std::{
    error::Error,
    net::SocketAddr,
    time::{Duration, Instant},
};

use bytes::Bytes;
use local_ip_address::list_afinet_netifas;
use reqwest::header::{HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use serde::Deserialize;
use std::str::FromStr;
use str0m::bwe::Bitrate;
use str0m::media::MediaTime;
use str0m::stats::PeerStats;
use str0m::{
    change::SdpAnswer,
    format::Codec,
    media::{Direction as RtcDirection, MediaKind, Mid},
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
    Connected,
    Stats(PeerStats),
    Disconnected,
}

#[derive(Debug)]
pub enum Direction {
    Publish,
    _Subscribe,
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
    url: String,
    token: Option<String>,
    buf: [u8; 1500],
    video_mid: Option<Mid>,
    _audio_mid: Option<Mid>,
}

impl Client {
    pub async fn new(url: &str, token: &Option<String>) -> Result<Self, WebrtcError> {
        let socket = UdpSocket::bind("0.0.0.0:0".parse::<SocketAddrV4>().unwrap())
            .await
            .expect("Should bind udp socket");

        let mut rtc = Rtc::builder()
            .clear_codecs()
            .enable_h264(true)
            // .enable_opus(true)
            .set_rtp_mode(false)
            .set_stats_interval(Some(Duration::from_secs(2)))
            .enable_bwe(Some(Bitrate::kbps(10000)))
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
                            local_socket_addr = Some(socket_addr.clone());
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
            url: url.to_string(),
            token: token.to_owned(),
            buf: [0; 1500],
            video_mid: None,
            _audio_mid: None,
        })
    }

    pub async fn prepare(&mut self, direction: Direction) -> Result<(), WebrtcError> {
        let direction = match direction {
            Direction::Publish => RtcDirection::SendOnly,
            Direction::_Subscribe => RtcDirection::RecvOnly,
        };

        // Add receive tracks and generate an offer
        let mut change = self.rtc.sdp_api();

        // TODO: Stream ID and Track ID should be UUIDs or something
        // self.audio_mid = Some(change.add_media(
        //     MediaKind::Audio,
        //     direction,
        //     Some("audio_0".to_string()),
        //     Some("audio_0".to_string()),
        // ));

        self.video_mid = Some(change.add_media(
            MediaKind::Video,
            direction,
            Some("video_0".to_string()),
            Some("video_0".to_string()),
        ));

        let (offer, pending) = change.apply().ok_or(WebrtcError::SdpError)?;

        let offer_str = offer.to_sdp_string();
        info!("offer: {}", offer_str);
        info!("token: {:?}", self.token);
        info!("url: {}", self.url);

        let mut headers = reqwest::header::HeaderMap::new();

        if let Some(token) = &self.token {
            let authoriation_value = HeaderValue::from_str(&format!("Bearer {}", token))
                .map_err(|e| WebrtcError::ServerError(e.into()))?;
            headers.append(AUTHORIZATION, authoriation_value);
        }

        headers.append(
            CONTENT_TYPE,
            HeaderValue::from_str("application/sdp").unwrap(),
        );
        headers.append(
            USER_AGENT,
            HeaderValue::from_str("lambda-test-client").unwrap(),
        );
        headers.append(ACCEPT, HeaderValue::from_str("application/sdp").unwrap());

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| WebrtcError::ServerError(e.into()))?;

        let mut next_url =
            reqwest::Url::from_str(&self.url).map_err(|e| WebrtcError::ServerError(e.into()))?;
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

        // remove a=framerate\n because it causes parsing problems
        let answer = answer.replace("a=framerate:60\n", "");
        let answer = SdpAnswer::from_sdp_string(&answer).map_err(|_| WebrtcError::SdpError)?;

        self.rtc
            .sdp_api()
            .accept_answer(pending, answer)
            .map_err(|_| WebrtcError::SdpError)?;

        Ok(())
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
                    return Ok(WebrtcEvent::Connected);
                }
                Event::IceConnectionStateChange(state) => {
                    info!("[WhepClient] ice connection state change: {:?}", state);
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
                    return Ok(WebrtcEvent::Stats(stats));
                }
                Event::RtpPacket(pkt) => {
                    trace!("rtp packet: {:?}", pkt);
                    return Ok(WebrtcEvent::Continue);
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
                    error!("[WhepClient] error handle input rtc: {:?}", e);
                    Ok(WebrtcEvent::Continue)
                }
            };
        }

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
            Ok(Err(e)) => {
                error!("[TransportWebrtc] network error {:?}", e);
                return Err(WebrtcError::NetworkError(e.into()));
            }
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

    pub fn _send_audio(&mut self, frame_data: Bytes, pts: Duration) -> Result<(), WebrtcError> {
        if let Some(mid) = self._audio_mid {
            let params = &self
                .rtc
                .codec_config()
                .find(|p| p.spec().codec == Codec::Opus)
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
                    .map_err(|e| WebrtcError::WebrtcError(e.into()))?;
            }
        } else {
            warn!("trying to send video without mid");
        }
        Ok(())
    }
}
