use crate::client::WebrtcError;
use std::net::IpAddr;
use std::{
    io::ErrorKind,
    net::UdpSocket,
    thread,
    time::{Duration, Instant},
};
use str0m::{
    change::SdpOffer,
    net::{Protocol, Receive},
    Candidate, Event, IceConnectionState, Input, Output, Rtc,
};
use systemstat::{Platform, System};

pub struct Player {
    pub answer: String,
}

fn run(mut rtc: Rtc, socket: UdpSocket) {
    let mut buf = Vec::new();
    loop {
        let timeout = match rtc.poll_output().expect("Ok") {
            Output::Timeout(v) => v,
            Output::Transmit(v) => {
                socket.send_to(&v.contents, v.destination).expect("Ok");
                continue;
            }
            Output::Event(v) => {
                if v == Event::IceConnectionStateChange(IceConnectionState::Disconnected) {
                    panic!("ICE has disconnected")
                }
                continue;
            }
        };

        let timeout = timeout - Instant::now();
        if timeout.is_zero() {
            rtc.handle_input(Input::Timeout(Instant::now()))
                .expect("Ok");
            continue;
        }

        socket.set_read_timeout(Some(timeout)).expect("Ok");
        buf.resize(2000, 0);

        let input = match socket.recv_from(&mut buf) {
            Ok((n, source)) => {
                buf.truncate(n);
                Input::Receive(
                    Instant::now(),
                    Receive {
                        proto: Protocol::Udp,
                        source,
                        destination: socket.local_addr().unwrap(),
                        contents: buf.as_slice().try_into().expect("Ok"),
                    },
                )
            }

            Err(e) => match e.kind() {
                // Expected error for set_read_timeout(). One for windows, one for the rest.
                ErrorKind::WouldBlock | ErrorKind::TimedOut => Input::Timeout(Instant::now()),
                _ => panic!("{}", e),
            },
        };

        rtc.handle_input(input).expect("Ok");
    }
}

impl Player {
    pub fn new(offer: String) -> Result<Self, WebrtcError> {
        let mut rtc = Rtc::builder()
            .clear_codecs()
            .enable_h264(true)
            .set_rtp_mode(false)
            .set_stats_interval(Some(Duration::from_secs(2)))
            .build();

        let host_addr = select_host_address();

        let socket = UdpSocket::bind(format!("{host_addr}:0")).expect("binding a random UDP port");
        let addr = socket.local_addr().expect("a local socket adddress");
        let candidate = Candidate::host(addr, "udp").expect("a host candidate");
        rtc.add_local_candidate(candidate);

        let offer = SdpOffer::from_sdp_string(&offer).map_err(|_| WebrtcError::SdpError)?;
        if let Ok(answer) = rtc.sdp_api().accept_offer(offer) {
            thread::spawn(|| {
                run(rtc, socket);
            });

            return Ok(Self {
                answer: answer.to_sdp_string(),
            });
        }

        return Err(WebrtcError::SdpError);
    }
}

pub fn select_host_address() -> IpAddr {
    let system = System::new();
    let networks = system.networks().unwrap();

    for net in networks.values() {
        for n in &net.addrs {
            if let systemstat::IpAddr::V4(v) = n.addr {
                if !v.is_loopback() && !v.is_link_local() && !v.is_broadcast() {
                    return IpAddr::V4(v);
                }
            }
        }
    }

    panic!("Found no usable network interface");
}
