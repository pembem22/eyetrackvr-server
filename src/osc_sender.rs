use async_broadcast::Receiver;
use rosc::{encoder, OscMessage, OscPacket, OscType};
use tokio::net::UdpSocket;
use tokio_stream::StreamExt;

use crate::inference::EyeState;

pub fn start_osc_sender(
    mut rx: Receiver<(EyeState, EyeState)>,
    osc_out_address: String,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let sock = UdpSocket::bind("0.0.0.0:0").await.unwrap();
        sock.connect(osc_out_address).await.unwrap();

        const VRCHAT_NATIVE: bool = true;
        const VRCFT_V2: bool = true;

        while let Some((l, r)) = rx.next().await {
            if VRCHAT_NATIVE {
                const SEND_EYES_CLOSED: bool = true;

                sock.send(
                    &encoder::encode(&OscPacket::Message(OscMessage {
                        addr: "/tracking/eye/LeftRightPitchYaw".to_string(),
                        args: vec![
                            OscType::Float(l.pitch),
                            OscType::Float(l.yaw),
                            OscType::Float(r.pitch),
                            OscType::Float(r.yaw),
                        ],
                    }))
                    .unwrap(),
                )
                .await.unwrap();

                if SEND_EYES_CLOSED {
                    sock.send(
                        &encoder::encode(&OscPacket::Message(OscMessage {
                            addr: "/tracking/eye/EyesClosedAmount".to_string(),
                            args: vec![OscType::Float(1.0 - (l.openness + r.openness) / 2.0)],
                        }))
                        .unwrap(),
                    )
                    .await.unwrap();
                }
            }

            if VRCFT_V2 {
                sock.send(
                    &encoder::encode(&OscPacket::Message(OscMessage {
                        addr: "/avatar/parameters/v2/EyeLeftX".to_string(),
                        args: vec![OscType::Float(l.yaw / 90.0)],
                    }))
                    .unwrap(),
                )
                .await.unwrap();
                sock.send(
                    &encoder::encode(&OscPacket::Message(OscMessage {
                        addr: "/avatar/parameters/v2/EyeLeftY".to_string(),
                        args: vec![OscType::Float(-l.pitch / 90.0)],
                    }))
                    .unwrap(),
                )
                .await.unwrap();
                sock.send(
                    &encoder::encode(&OscPacket::Message(OscMessage {
                        addr: "/avatar/parameters/v2/EyeLidLeft".to_string(),
                        args: vec![OscType::Float(l.openness * 0.75)],
                    }))
                    .unwrap(),
                )
                .await.unwrap();

                sock.send(
                    &encoder::encode(&OscPacket::Message(OscMessage {
                        addr: "/avatar/parameters/v2/EyeRightX".to_string(),
                        args: vec![OscType::Float(r.yaw / 90.0)],
                    }))
                    .unwrap(),
                )
                .await.unwrap();
                sock.send(
                    &encoder::encode(&OscPacket::Message(OscMessage {
                        addr: "/avatar/parameters/v2/EyeRightY".to_string(),
                        args: vec![OscType::Float(-r.pitch / 90.0)],
                    }))
                    .unwrap(),
                )
                .await.unwrap();
                sock.send(
                    &encoder::encode(&OscPacket::Message(OscMessage {
                        addr: "/avatar/parameters/v2/EyeLidRight".to_string(),
                        args: vec![OscType::Float(r.openness * 0.75)],
                    }))
                    .unwrap(),
                )
                .await.unwrap();
            }
        }
    })
}
