use std::net::UdpSocket;
use std::ops::Sub;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use image::{DynamicImage, GenericImageView};
use onnxruntime::environment::Environment;
use onnxruntime::tensor::OrtOwnedTensor;
use onnxruntime::{ndarray, GraphOptimizationLevel, LoggingLevel, OrtError};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use rosc::encoder;
use rosc::{OscMessage, OscPacket, OscType};

use one_euro_rs::OneEuroFilter;

use crate::Frame;

pub fn start_onnx(
    l_frame_mutex: Arc<Mutex<Frame>>,
    r_frame_mutex: Arc<Mutex<Frame>>,
    sock: UdpSocket,
) -> Result<JoinHandle<()>, OrtError> {
    Ok(tokio::task::spawn_blocking(move || {
        let environment = Environment::builder()
            .with_name("test")
            .with_log_level(LoggingLevel::Verbose)
            .build()
            .unwrap();

        let mut l_session = environment
            .new_session_builder()
            .unwrap()
            .with_optimization_level(GraphOptimizationLevel::All)
            .unwrap()
            .with_number_threads(3)
            .unwrap()
            .with_model_from_file("model.onnx")
            .unwrap();

        let mut r_session = environment
            .new_session_builder()
            .unwrap()
            .with_optimization_level(GraphOptimizationLevel::All)
            .unwrap()
            .with_number_threads(3)
            .unwrap()
            .with_model_from_file("model.onnx")
            .unwrap();

        let start_timestamp = SystemTime::now().sub(Duration::from_secs(1));

        let mut l_last_timestamp = SystemTime::now();
        let mut r_last_timestamp = SystemTime::now();

        const FREQ: f32 = 70.8;
        const BETA: f32 = 0.3;
        const FCMIN: f32 = 1.0;
        const FOV: f32 = 90.0;

        let mut l_p_oe = OneEuroFilter::new(FREQ, FCMIN, FCMIN, BETA);
        let mut l_y_oe = OneEuroFilter::new(FREQ, FCMIN, FCMIN, BETA);
        let mut l_e_oe = OneEuroFilter::new(FREQ, FCMIN, FCMIN, BETA);
        let mut r_p_oe = OneEuroFilter::new(FREQ, FCMIN, FCMIN, BETA);
        let mut r_y_oe = OneEuroFilter::new(FREQ, FCMIN, FCMIN, BETA);
        let mut r_e_oe = OneEuroFilter::new(FREQ, FCMIN, FCMIN, BETA);

        loop {
            let frame = l_frame_mutex.blocking_lock();
            let l_timestamp = frame.timestamp;
            if l_last_timestamp == l_timestamp {
                continue;
            }

            // println!("{:2.3}", 1000f32 / (l_timestamp.duration_since(last_timestamp).unwrap().as_millis() as f32));

            l_last_timestamp = l_timestamp;

            let array = ndarray::Array::from_iter(frame.decoded.pixels().map(|p| p.0[0] as f32));
            drop(frame);

            let array = array.into_shape((1, 240, 240, 1)).unwrap();

            let input_tensor = vec![array];
            let outputs: Vec<OrtOwnedTensor<f32, _>> = l_session.run(input_tensor).unwrap();

            // Collecting with iterator works, using by index throws out of bounds...
            let l_pitch_yaw = outputs[0].iter().collect::<Vec<_>>();

            let frame = r_frame_mutex.blocking_lock();
            let mut r_timestamp = frame.timestamp;

            let mirrored_frame = DynamicImage::from(frame.decoded.clone()).fliph();
            drop(frame);

            if r_last_timestamp == r_timestamp {
                r_timestamp = r_timestamp + Duration::from_millis(1);
            }

            r_last_timestamp = r_timestamp;

            let array =
                ndarray::Array::from_iter(mirrored_frame.pixels().map(|p| p.2 .0[0] as f32));

            let array = array.into_shape((1, 240, 240, 1)).unwrap();

            let input_tensor = vec![array];
            let outputs: Vec<OrtOwnedTensor<f32, _>> = r_session.run(input_tensor).unwrap();

            // Collecting with iterator works, using by index throws out of bounds...
            let r_pitch_yaw = outputs[0].iter().collect::<Vec<_>>();

            let l_pitch = l_p_oe.filter_with_timestamp(
                *l_pitch_yaw[0],
                l_timestamp
                    .duration_since(start_timestamp)
                    .unwrap()
                    .as_secs_f32(),
            );
            let l_yaw = l_y_oe.filter_with_timestamp(
                *l_pitch_yaw[1],
                l_timestamp
                    .duration_since(start_timestamp)
                    .unwrap()
                    .as_secs_f32(),
            );
            let l_eyelid = l_e_oe.filter_with_timestamp(
                *l_pitch_yaw[2],
                l_timestamp
                    .duration_since(start_timestamp)
                    .unwrap()
                    .as_secs_f32(),
            );
            let r_pitch = r_p_oe.filter_with_timestamp(
                *r_pitch_yaw[0],
                r_timestamp
                    .duration_since(start_timestamp)
                    .unwrap()
                    .as_secs_f32(),
            );
            let r_yaw = r_y_oe.filter_with_timestamp(
                -r_pitch_yaw[1],
                r_timestamp
                    .duration_since(start_timestamp)
                    .unwrap()
                    .as_secs_f32(),
            );
            let r_eyelid = r_e_oe.filter_with_timestamp(
                *r_pitch_yaw[2],
                r_timestamp
                    .duration_since(start_timestamp)
                    .unwrap()
                    .as_secs_f32(),
            );

            let msg_buf = encoder::encode(&OscPacket::Message(OscMessage {
                addr: "/tracking/eye/LeftRightPitchYaw".to_string(),
                args: vec![
                    OscType::Float(l_pitch),
                    OscType::Float(l_yaw),
                    OscType::Float(r_pitch),
                    OscType::Float(r_yaw),
                ],
            }))
            .unwrap();

            // println!(
            //     "{:3.3} {:3.3} {:3.3} {:3.3}",
            //     l_pitch, l_pitch_yaw[0], r_pitch, r_pitch_yaw[0]
            // );

            sock.send(&msg_buf).unwrap();

            let msg_buf = encoder::encode(&OscPacket::Message(OscMessage {
                addr: "/tracking/eye/EyesClosedAmount".to_string(),
                args: vec![OscType::Float(1.0 - (l_eyelid + r_eyelid) / 2.0)],
            }))
            .unwrap();

            // println!(
            //     "{:3.3} {:3.3} {:3.3} {:3.3}",
            //     l_yaw, l_pitch_yaw[1], r_yaw, r_pitch_yaw[1]
            // );

            sock.send(&msg_buf).unwrap();
        }
    }))
}
// struct ONNX<'a> {
//     environment: Environment,
//     l_session: Session<'a>,
// }

// impl<'a> ONNX<'_> {
//     pub fn new() -> Result<ONNX<'a>, OrtError> {
// let environment = Environment::builder()
//     .with_name("test")
//     .with_log_level(LoggingLevel::Verbose)
//     .build()?;

// let mut session = environment
//     .new_session_builder()?
//     .with_optimization_level(GraphOptimizationLevel::All)?
//     .with_number_threads(3)?
//     .with_model_from_file("model.onnx")?;

//         Ok(ONNX { l_session: session })
//     }
// }
