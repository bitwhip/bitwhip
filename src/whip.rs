use crate::client::{Client, WebrtcEvent};
use crate::EncodedPacket;
use bytes::Bytes;
use futures::executor;
use std::time::Instant;
use tokio::sync::mpsc::{error::TryRecvError, UnboundedReceiver};
use tracing::{error, info};

pub async fn publish(
    publish_url: &str,
    token: Option<String>,
    mut packet_rx: UnboundedReceiver<EncodedPacket>,
) {
    info!(
        "creating client to push to {} with token: {:?}",
        publish_url, token
    );

    let mut client = Client::new().await.unwrap();
    client
        .send_whip_request(&publish_url, &token)
        .await
        .expect("should connect");

    loop {
        match client.recv().await {
            Ok(event) => match event {
                WebrtcEvent::Connected => {
                    info!("connected");
                }
                WebrtcEvent::Disconnected => {
                    info!("disconnected");
                    break;
                }
                WebrtcEvent::Stats(stats) => {
                    info!("stats: {:?}", stats);
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
                error!("error: {:?}", err);
                break;
            }
        }
    }
}

pub fn subscribe(offer: String) -> String {
    let mut client = executor::block_on(Client::new()).expect("Ok");
    let answer = client.accept_whip_request(offer).expect("Ok");

    tokio::task::spawn(async move {
         loop {
             match client.recv().await {
                 Ok(event) => match event {
                     WebrtcEvent::Connected => {
                         info!("connected");
                     }
                     WebrtcEvent::Disconnected => {
                         info!("disconnected");
                         break;
                     }
                     WebrtcEvent::Stats(stats) => {
                         info!("stats: {:?}", stats);
                     }
                     WebrtcEvent::Continue => {
                         info!("Continue");
                     }
                 },
                 Err(err) => {
                     error!("error: {:?}", err);
                     break;
                 }
             }
         }
    });

    answer
}
