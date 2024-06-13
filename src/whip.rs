use crate::client::{Client, Direction, WebrtcEvent};
use crate::EncodedPacket;
use bytes::Bytes;
use std::time::Instant;
use tokio::sync::mpsc::{error::TryRecvError, UnboundedReceiver};
use tracing::{error, info};

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

    loop {
        match client.recv().await {
            Ok(event) => match event {
                WebrtcEvent::Connected => {
                    info!("[WhepClient] connected");
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
            },
            Err(err) => {
                error!("[WhepClient] error: {:?}", err);
                break;
            }
        }
    }
}
