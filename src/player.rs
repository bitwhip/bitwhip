use crate::client::{WebrtcError};
use std::net::{IpAddr, SocketAddrV4};
use std::{
    net::SocketAddr,
    time::{Duration},
};

use local_ip_address::list_afinet_netifas;
use str0m::bwe::Bitrate;
use str0m::{
    Candidate, Rtc,
    change::SdpOffer,
};
use std::net::UdpSocket;

pub struct Player {
    rtc: Rtc,
    pub answer: String,
}

impl Player {
    pub fn new(offer: String) -> Result<Self, WebrtcError> {
        let socket = UdpSocket::bind("0.0.0.0:0".parse::<SocketAddrV4>().unwrap())
            .expect("Should bind udp socket");

        let mut rtc = Rtc::builder()
            .clear_codecs()
            .enable_h264(true)
            .set_rtp_mode(false)
            .set_stats_interval(Some(Duration::from_secs(2)))
            .enable_bwe(Some(Bitrate::kbps(10000)))
            .build();

        let mut local_socket_addr = None;
        if let Ok(network_interfaces) = list_afinet_netifas() {
            for (name, ip) in network_interfaces {
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

        let offer = SdpOffer::from_sdp_string(&offer).map_err(|_| WebrtcError::SdpError)?;
        if let Ok(answer) = rtc.sdp_api().accept_offer(offer) {
            return Ok(Self {
                answer: answer.to_sdp_string(),
                rtc,
            })
        }

        return Err(WebrtcError::SdpError);
    }
}
