use std::ops::Sub;
use std::time::{Duration, SystemTime};

use async_broadcast::{broadcast, Receiver, RecvError, Sender};
use image::{DynamicImage, GenericImageView};
use ort::session::{builder::GraphOptimizationLevel, Session};
use tokio::runtime::Handle;
use tokio::task::JoinHandle;

use one_euro_rs::OneEuroFilter;
use tokio_stream::StreamExt;

use crate::{Eye, Frame};

#[derive(Copy, Clone, Debug, Default)]
pub struct EyeState {
    pub pitch: f32,
    pub yaw: f32,
    pub openness: f32,
}

pub fn start_inference(
    l_rx: Receiver<Frame>,
    r_rx: Receiver<Frame>,
    out_tx: Sender<(EyeState, EyeState)>,
    model_path: String,
    threads_per_eye: usize,
) -> JoinHandle<()> {
    let (l_eye_tx, l_eye_rx) = broadcast::<EyeState>(1);
    let (r_eye_tx, r_eye_rx) = broadcast::<EyeState>(1);

    inference_task(l_rx, &model_path, threads_per_eye, l_eye_tx, Eye::L);
    inference_task(r_rx, &model_path, threads_per_eye, r_eye_tx, Eye::R);

    let l_eye_rx = l_eye_rx.map(|es| (Eye::L, es));
    let r_eye_rx = r_eye_rx.map(|es| (Eye::R, es));

    let mut eyes_rx = l_eye_rx.merge(r_eye_rx);

    tokio::spawn(async move {
        let mut l_eye_state: EyeState = Default::default();
        let mut r_eye_state: EyeState = Default::default();

        while let Some((eye, state)) = eyes_rx.next().await {
            println!("{:?} {:?}", eye, state);

            match eye {
                Eye::L => l_eye_state = state,
                Eye::R => r_eye_state = state,
            };

            out_tx
                .broadcast_direct((l_eye_state, r_eye_state))
                .await
                .unwrap();
        }
    })
}

pub fn inference_task(
    mut rx: Receiver<Frame>,
    model_path: &str,
    threads: usize,
    tx: Sender<EyeState>,
    eye: Eye,
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
        let model = Session::builder()
            .unwrap()
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .unwrap()
            .with_intra_threads(threads)
            .unwrap()
            .commit_from_file(model_path)
            .unwrap();

        let handle = Handle::current();

        loop {
            let frame = handle.block_on(async {
                match rx.recv_direct().await {
                    Ok(frame) => Some(frame),
                    Err(e) => {
                        match e {
                            RecvError::Overflowed(skipped) => println!("Skipped {skipped} frames"),
                            RecvError::Closed => println!("Channel closed"),
                        };
                        None
                    }
                }
            });

            let frame = match frame {
                Some(frame) => frame,
                None => continue,
            };

            let mut raw_frame: DynamicImage = image::DynamicImage::ImageRgb8(frame.decoded);

            if eye == Eye::R {
                raw_frame = raw_frame.fliph();
            }

            let cropped_frame = raw_frame.view(30, 30, 180, 180);

            let final_frame = image::imageops::resize(
                &cropped_frame.to_image(),
                64,
                64,
                image::imageops::FilterType::Lanczos3,
            );

            let array = ndarray::Array::from_iter(final_frame.pixels().map(|p| p[0] as f32));

            let array = array.to_shape((1, 64, 64, 1)).unwrap();


            let outputs = model.run(ort::inputs![&array].unwrap()).unwrap();
            let output = outputs.iter().next().unwrap().1;
            let output = output.try_extract_tensor::<f32>().unwrap();

            let filter_secs = frame
                .timestamp
                .duration_since(start_timestamp)
                .unwrap()
                .as_secs_f32();

            let pitch = filter_pitch.filter_with_timestamp(output[0], filter_secs);
            let yaw = filter_yaw.filter_with_timestamp(output[1], filter_secs)
                * if eye == Eye::R { -1.0 } else { 1.0 };
            let openness = filter_openness.filter_with_timestamp(output[2], filter_secs);

            let _ = handle.block_on(async {
                tx.broadcast_direct(EyeState {
                    pitch,
                    yaw,
                    openness,
                })
                .await
            });
        }
    })
}
