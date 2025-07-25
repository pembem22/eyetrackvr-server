use std::time::SystemTime;

use async_broadcast::Receiver;
use const_format::concatcp;
use rosc::{OscBundle, OscMessage, OscPacket, OscType, encoder};
use tokio::net::UdpSocket;
use tokio_stream::StreamExt;

use crate::structs::EyeState;

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
                .await
                .unwrap();

                if SEND_EYES_CLOSED {
                    sock.send(
                        &encoder::encode(&OscPacket::Message(OscMessage {
                            addr: "/tracking/eye/EyesClosedAmount".to_string(),
                            args: vec![OscType::Float(1.0 - (l.eyelid + r.eyelid) / 2.0)],
                        }))
                        .unwrap(),
                    )
                    .await
                    .unwrap();
                }
            }

            if VRCFT_V2 {
                const VRCFT_OSC_PREFIX: &str = "/avatar/parameters/FT/v2/";

                let l_yaw_norm = l.yaw.to_radians().sin();
                let l_pitch_norm = l.pitch.to_radians().sin();
                let r_yaw_norm = r.yaw.to_radians().sin();
                let r_pitch_norm = r.pitch.to_radians().sin();
                let pitch_norm = ((l.pitch + r.pitch) / 2.0).to_radians().sin();

                sock.send(
                    &encoder::encode(&OscPacket::Bundle(OscBundle {
                        timetag: SystemTime::now().try_into().unwrap(),
                        content: vec![
                            OscPacket::Message(OscMessage {
                                addr: concatcp!(VRCFT_OSC_PREFIX, "EyeY").to_string(),
                                args: vec![OscType::Float(-pitch_norm)],
                            }),
                            OscPacket::Message(OscMessage {
                                addr: concatcp!(VRCFT_OSC_PREFIX, "EyeLeftX").to_string(),
                                args: vec![OscType::Float(l_yaw_norm)],
                            }),
                            OscPacket::Message(OscMessage {
                                addr: concatcp!(VRCFT_OSC_PREFIX, "EyeLeftY").to_string(),
                                args: vec![OscType::Float(-l_pitch_norm)],
                            }),
                            OscPacket::Message(OscMessage {
                                addr: concatcp!(VRCFT_OSC_PREFIX, "EyeLidLeft").to_string(),
                                args: vec![OscType::Float(l.eyelid * 0.75)],
                            }),
                            OscPacket::Message(OscMessage {
                                addr: concatcp!(VRCFT_OSC_PREFIX, "EyeRightX").to_string(),
                                args: vec![OscType::Float(r_yaw_norm)],
                            }),
                            OscPacket::Message(OscMessage {
                                addr: concatcp!(VRCFT_OSC_PREFIX, "EyeRightY").to_string(),
                                args: vec![OscType::Float(-r_pitch_norm)],
                            }),
                            OscPacket::Message(OscMessage {
                                addr: concatcp!(VRCFT_OSC_PREFIX, "EyeLidRight").to_string(),
                                args: vec![OscType::Float(r.eyelid * 0.75)],
                            }),
                        ],
                    }))
                    .unwrap(),
                )
                .await
                .unwrap();
            }
        }
    })
}
