use crate::client::{Client, Direction, WebrtcEvent, WhipClaims};
use crate::EncodedPacket;
use bytes::{Bytes, BytesMut};
use std::io::BufReader;
use std::time::Duration;
use std::{fs::File, time::Instant};
use tokio::sync::mpsc::{error::TryRecvError, UnboundedReceiver};
use tracing::{debug, error, info, trace};

fn find_nal_units(data: &[u8]) -> Vec<&[u8]> {
    let mut nal_units = Vec::new();
    let mut i = 0;

    while i + 3 <= data.len() {
        // Detect start code (0x000001 or 0x00000001)
        if data[i..i + 3] == [0, 0, 1] || (i + 4 <= data.len() && data[i..i + 4] == [0, 0, 0, 1]) {
            // Move past the start code
            let start = i;
            i += if data[i..i + 3] == [0, 0, 1] { 3 } else { 4 };

            // Search for the next start code to mark the end of this NAL unit
            while i + 3 <= data.len()
                && data[i..i + 3] != [0, 0, 1]
                && (i + 4 > data.len() || data[i..i + 4] != [0, 0, 0, 1])
            {
                i += 1;
            }

            nal_units.push(&data[start..i]);
        } else {
            i += 1;
        }
    }

    nal_units
}

pub async fn publish(
    publish_url: &str,
    token: Option<String>,
    mut packet_rx: UnboundedReceiver<EncodedPacket>,
) {
    info!(
        "creating WHEP client to push to {} with token: {:?}",
        publish_url, token
    );

    let mut client = Client::new(&publish_url, &token).await.unwrap(); // TODO error handling
    client
        .prepare(Direction::Publish)
        .await
        .expect("should connect");

    // once the client is connected, we operate the recv() loop
    let started = std::time::Instant::now();
    let mut connected = None;

    loop {
        match client.recv().await {
            Ok(event) => match event {
                WebrtcEvent::Connected => {
                    info!("[WhepClient] connected");
                    connected = Some(std::time::Instant::now());
                }
                WebrtcEvent::Disconnected => {
                    info!("[WhepClient] disconnected");
                    break;
                }
                WebrtcEvent::Stats(stats) => {
                    info!("[WhepClient] stats: {:?}", stats);
                }
                WebrtcEvent::Continue => {
                    // Here we should be clear to write data
                    // flush any media samples between the last sample index and the
                    // current stream offset
                    if let Some(start) = connected {
                        loop {
                            let packet = packet_rx.try_recv();
                            match packet {
                                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
                                Ok(packet) => {
                                    let pts = Instant::now() - packet.1;
                                    // let nal_units = find_nal_units(&packet.0.data);
                                    // for nalu in nal_units {
                                    //     if nalu[4] & 0b11111 == 7 {
                                    //         info!("KEY FRAME!!!!!!!");
                                    //     }
                                    // }
                                    if let Some(data) = packet.0.data() {
                                        client
                                            .send_video(Bytes::copy_from_slice(data), pts)
                                            .unwrap();
                                    }
                                }
                            }
                        }
                    }
                }
            },
            Err(err) => {
                error!("[WhepClient] error: {:?}", err);
                break;
            }
        }
    }
}
