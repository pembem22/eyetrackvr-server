use std::time::SystemTime;

use async_broadcast::Receiver;
use const_format::concatcp;
use rosc::{OscBundle, OscMessage, OscPacket, OscType, encoder};
use tokio::net::UdpSocket;
use tokio_stream::StreamExt;

use crate::structs::CombinedEyeGazeState;

pub fn start_osc_sender(
    mut rx: Receiver<CombinedEyeGazeState>,
    osc_out_address: String,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let sock = UdpSocket::bind("0.0.0.0:0").await.unwrap();
        sock.connect(osc_out_address).await.unwrap();

        const VRCHAT_NATIVE: bool = true;
        const VRCFT_V2: bool = true;

        while let Some(combined_eyes) = rx.next().await {
            if VRCHAT_NATIVE {
                const SEND_EYES_CLOSED: bool = true;

                sock.send(
                    &encoder::encode(&OscPacket::Message(OscMessage {
                        addr: "/tracking/eye/LeftRightPitchYaw".to_string(),
                        args: vec![
                            OscType::Float(combined_eyes.pitch),
                            OscType::Float(combined_eyes.l_yaw),
                            OscType::Float(combined_eyes.pitch),
                            OscType::Float(combined_eyes.r_yaw),
                        ],
                    }))
                    .unwrap(),
                )
                .await
                .unwrap();

                if SEND_EYES_CLOSED {
                    let vrc_eyelids = f32::clamp(
                        1.0 - (combined_eyes.l_eyelid + combined_eyes.r_eyelid) / 0.75 / 2.0,
                        0.0,
                        1.0,
                    );
                    sock.send(
                        &encoder::encode(&OscPacket::Message(OscMessage {
                            addr: "/tracking/eye/EyesClosedAmount".to_string(),
                            args: vec![OscType::Float(vrc_eyelids)],
                        }))
                        .unwrap(),
                    )
                    .await
                    .unwrap();
                }
            }

            if VRCFT_V2 {
                const VRCFT_OSC_PREFIX: &str = "/avatar/parameters/FT/v2/";

                let l_yaw_norm = combined_eyes.l_yaw.to_radians().sin();
                let l_pitch_norm = combined_eyes.pitch.to_radians().sin();
                let l_eyelid = combined_eyes.l_eyelid;

                let r_yaw_norm = combined_eyes.r_yaw.to_radians().sin();
                let r_pitch_norm = combined_eyes.pitch.to_radians().sin();
                let r_eyelid = combined_eyes.r_eyelid;
                let pitch_norm = ((combined_eyes.pitch + combined_eyes.pitch) / 2.0)
                    .to_radians()
                    .sin();

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
                                args: vec![OscType::Float(l_eyelid)],
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
                                args: vec![OscType::Float(r_eyelid)],
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
