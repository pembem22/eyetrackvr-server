use std::net::UdpSocket;
use std::ops::Sub;
use std::time::{Duration, SystemTime};

use onnxruntime::environment::Environment;
use onnxruntime::tensor::OrtOwnedTensor;
use onnxruntime::{ndarray, GraphOptimizationLevel, LoggingLevel, OrtError};
use postage::broadcast::{self, Receiver, Sender};
use postage::sink::Sink;
use postage::stream::Stream;
use tokio::task::JoinHandle;

use rosc::encoder;
use rosc::{OscMessage, OscPacket, OscType};

use one_euro_rs::OneEuroFilter;

use crate::{Eye, Frame};

#[derive(Clone, Debug, Default)]
pub struct EyeState {
    pitch: f32,
    yaw: f32,
    openness: f32,
}

pub fn start_onnx(
    l_rx: Receiver<Frame>,
    r_rx: Receiver<Frame>,
    sock: UdpSocket,
    model_path: String,
) -> Result<JoinHandle<()>, OrtError> {
    let (l_eye_tx, l_eye_rx) = broadcast::channel::<EyeState>(2);
    let (r_eye_tx, r_eye_rx) = broadcast::channel::<EyeState>(2);

    inference_task(l_rx, &model_path, l_eye_tx);
    inference_task(r_rx, &model_path, r_eye_tx);

    let l_eye_rx = l_eye_rx.map(|es| (Eye::L, es));
    let r_eye_rx = r_eye_rx.map(|es| (Eye::R, es));

    let mut eyes_rx = l_eye_rx.merge(r_eye_rx);

    Ok(tokio::spawn(async move {
        let mut l_eye_state: EyeState = Default::default();
        let mut r_eye_state: EyeState = Default::default();

        while let Some((eye, state)) = eyes_rx.recv().await {
            println!("{:?} {:?}", eye, state);

            match eye {
                Eye::L => l_eye_state = state,
                Eye::R => r_eye_state = state,
            };

            let l = &l_eye_state;
            let r = &r_eye_state;

            const VRCHAT_NATIVE: bool = true;
            const VRCFT_V2: bool = false;

            if VRCHAT_NATIVE {
                let msg_buf = encoder::encode(&OscPacket::Message(OscMessage {
                    addr: "/tracking/eye/LeftRightPitchYaw".to_string(),
                    args: vec![
                        OscType::Float(l.pitch),
                        OscType::Float(l.yaw),
                        OscType::Float(r.pitch),
                        OscType::Float(r.yaw),
                    ],
                }))
                .unwrap();
                sock.send(&msg_buf).unwrap();

                let msg_buf = encoder::encode(&OscPacket::Message(OscMessage {
                    addr: "/tracking/eye/EyesClosedAmount".to_string(),
                    args: vec![OscType::Float(1.0 - (l.openness + r.openness) / 2.0)],
                }))
                .unwrap();
                sock.send(&msg_buf).unwrap();
            }

            if VRCFT_V2 {
                sock.send(
                    &encoder::encode(&OscPacket::Message(OscMessage {
                        addr: "/avatar/parameters/v2/EyeLeftX".to_string(),
                        args: vec![OscType::Float(l.yaw / 90.0)],
                    }))
                    .unwrap(),
                )
                .unwrap();
                sock.send(
                    &encoder::encode(&OscPacket::Message(OscMessage {
                        addr: "/avatar/parameters/v2/EyeLeftY".to_string(),
                        args: vec![OscType::Float(-l.pitch / 90.0)],
                    }))
                    .unwrap(),
                )
                .unwrap();
                sock.send(
                    &encoder::encode(&OscPacket::Message(OscMessage {
                        addr: "/avatar/parameters/v2/EyeLidLeft".to_string(),
                        args: vec![OscType::Float(l.openness * 0.75)],
                    }))
                    .unwrap(),
                )
                .unwrap();

                sock.send(
                    &encoder::encode(&OscPacket::Message(OscMessage {
                        addr: "/avatar/parameters/v2/EyeRightX".to_string(),
                        args: vec![OscType::Float(r.yaw / 90.0)],
                    }))
                    .unwrap(),
                )
                .unwrap();
                sock.send(
                    &encoder::encode(&OscPacket::Message(OscMessage {
                        addr: "/avatar/parameters/v2/EyeRightY".to_string(),
                        args: vec![OscType::Float(-r.pitch / 90.0)],
                    }))
                    .unwrap(),
                )
                .unwrap();
                sock.send(
                    &encoder::encode(&OscPacket::Message(OscMessage {
                        addr: "/avatar/parameters/v2/EyeLidRight".to_string(),
                        args: vec![OscType::Float(r.openness * 0.75)],
                    }))
                    .unwrap(),
                )
                .unwrap();
            }
        }
    }))
}

pub fn inference_task(
    mut rx: Receiver<Frame>,
    model_path: &str,
    mut tx: Sender<EyeState>,
) -> JoinHandle<()> {
    const PY_BETA: f32 = 0.3;
    const PY_FCMIN: f32 = 0.5;
    let mut filter_pitch = OneEuroFilter::new(0.0, PY_FCMIN, PY_FCMIN, PY_BETA);
    let mut filter_yaw = OneEuroFilter::new(0.0, PY_FCMIN, PY_FCMIN, PY_BETA);

    const EL_BETA: f32 = 1.0;
    const EL_FCMIN: f32 = 3.0;
    let mut filter_openness = OneEuroFilter::new(0.0, EL_FCMIN, EL_FCMIN, EL_BETA);

    let start_timestamp = SystemTime::now().sub(Duration::from_secs(1));

    let model_path = model_path.to_owned();

    tokio::task::spawn_blocking(move || {
        // Cannot share the environment between threads.
        let environment = Environment::builder()
            .with_name("test")
            .with_log_level(LoggingLevel::Verbose)
            .build()
            .unwrap();

        let mut session = environment
            .new_session_builder()
            .unwrap()
            .with_optimization_level(GraphOptimizationLevel::All)
            .unwrap()
            .with_number_threads(3)
            .unwrap()
            .with_model_from_file(model_path)
            .unwrap();

        loop {
            let frame = match rx.try_recv() {
                Ok(mut frame) => {
                    let mut frames_skipped = 0;
                    while let Ok(new_frame) = rx.try_recv() {
                        frame = new_frame;
                        frames_skipped += 1;
                    }

                    if frames_skipped > 0 {
                        println!("Skipped {frames_skipped} frame(s)");
                    }
                    Some(frame)
                }
                Err(_) => rx.blocking_recv(),
            };

            let frame = match frame {
                Some(frame) => frame,
                None => {
                    println!("Got an empty frame");
                    continue;
                }
            };

            let array = ndarray::Array::from_iter(frame.decoded.pixels().map(|p| p.0[0] as f32));

            let array = array.into_shape((1, 240, 240, 1)).unwrap();

            let input_tensor = vec![array];
            let output: Vec<OrtOwnedTensor<f32, _>> = session.run(input_tensor).unwrap();

            // Collecting with iterator works, using by index throws out of bounds...
            let output = output[0].iter().collect::<Vec<_>>();

            let filter_secs = frame
                .timestamp
                .duration_since(start_timestamp)
                .unwrap()
                .as_secs_f32();

            let pitch = filter_pitch.filter_with_timestamp(*output[0], filter_secs);
            let yaw = filter_yaw.filter_with_timestamp(*output[1], filter_secs);
            let openness = filter_openness.filter_with_timestamp(*output[2], filter_secs);

            tx.blocking_send(EyeState {
                pitch,
                yaw,
                openness,
            })
            .unwrap();
        }
    })
}
