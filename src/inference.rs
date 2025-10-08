use async_broadcast::{Receiver, RecvError, Sender};
use image::{DynamicImage, GenericImageView};
use log::{error, warn};
use ort::{
    session::{Session, builder::GraphOptimizationLevel},
    value::TensorRef,
};
use tokio::task::JoinHandle;

use crate::structs::{Eye, EyeGazeState, EyesGazeState};
use crate::structs::{EyesFrame, EyesFrameType};

pub const FRAME_CROP_X: u32 = 30;
pub const FRAME_CROP_Y: u32 = 30;
pub const FRAME_CROP_W: u32 = 180;
pub const FRAME_CROP_H: u32 = 180;
pub const FRAME_RESIZE_W: u32 = 64;
pub const FRAME_RESIZE_H: u32 = 64;

pub fn eye_inference(
    mut rx: Receiver<EyesFrame>,
    tx: Sender<EyesGazeState>,
    #[cfg(not(target_os = "android"))] model_path: &str,
    threads: usize,
) -> JoinHandle<()> {
    #[cfg(not(target_os = "android"))]
    let model_path = model_path.to_owned();

    tokio::task::spawn_blocking(move || {
        let session_builder = Session::builder()
            .unwrap()
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .unwrap()
            .with_intra_threads(threads)
            .unwrap();

        let mut model = {
            #[cfg(target_os = "android")]
            {
                const MODEL_BYTES: &[u8] = include_bytes!("../model.onnx");
                session_builder.commit_from_memory_directly(MODEL_BYTES)
            }

            #[cfg(not(target_os = "android"))]
            {
                session_builder.commit_from_file(model_path)
            }
        }
        .unwrap();

        loop {
            let eyes_frame = loop {
                match rx.recv_blocking() {
                    Ok(eyes_frame) => break eyes_frame,
                    Err(e) => match e {
                        RecvError::Overflowed(skipped) => {
                            warn!("Skipped {skipped} frames");
                            continue;
                        }
                        RecvError::Closed => {
                            error!("Channel closed");
                            return;
                        }
                    },
                }
            };

            let mut run_eye_inference = |frame_view: image::SubImage<
                &image::ImageBuffer<image::Rgb<u8>, Vec<u8>>,
            >,
                                         is_left: bool| {
                // TODO: make it grayscale for a bit less operations? But crashes for some reason so far.
                // let mut frame_view = DynamicImage::ImageRgb8(frame_view.to_image()).grayscale();
                let mut frame_view = DynamicImage::ImageRgb8(frame_view.to_image());

                if !is_left {
                    frame_view = frame_view.fliph();
                }

                let cropped_frame =
                    frame_view.view(FRAME_CROP_X, FRAME_CROP_Y, FRAME_CROP_W, FRAME_CROP_H);

                let final_frame = image::imageops::resize(
                    &cropped_frame.to_image(),
                    FRAME_RESIZE_W,
                    FRAME_RESIZE_H,
                    image::imageops::FilterType::Lanczos3,
                );

                // Panics that the shape is wrong when using this.
                // let array = ndarray::Array::from_vec(final_frame.into_vec());
                let array = ndarray::Array::from_iter(final_frame.pixels().map(|p| p[0] as f32));

                let array = array
                    .to_shape((1, FRAME_RESIZE_W as usize, FRAME_RESIZE_H as usize, 1))
                    .unwrap();

                let tensor = TensorRef::from_array_view(&array).unwrap();

                let outputs = model.run(ort::inputs![tensor]).unwrap();
                let output = outputs.iter().next().unwrap().1;
                let output = output.try_extract_tensor::<f32>().unwrap();
                let output = output.1;

                EyeGazeState {
                    pitch: output[0],
                    yaw: output[1] * if is_left { 1.0 } else { -1.0 },
                    eyelid: output[2],
                }
            };

            match eyes_frame.frame_type {
                EyesFrameType::Both => {
                    let l_state = run_eye_inference(eyes_frame.get_left_view().unwrap(), true);
                    let r_state: EyeGazeState =
                        run_eye_inference(eyes_frame.get_right_view().unwrap(), false);

                    tx.broadcast_blocking(EyesGazeState::Both {
                        l_state,
                        r_state,
                        timestamp: eyes_frame.frame.timestamp,
                    })
                    .unwrap();
                }
                EyesFrameType::Left => {
                    let l_state = run_eye_inference(eyes_frame.get_left_view().unwrap(), true);

                    tx.broadcast_blocking(EyesGazeState::Mono {
                        eye: Eye::L,
                        state: l_state,
                        timestamp: eyes_frame.frame.timestamp,
                    })
                    .unwrap();
                }
                EyesFrameType::Rigth => {
                    let r_state = run_eye_inference(eyes_frame.get_right_view().unwrap(), false);

                    tx.broadcast_blocking(EyesGazeState::Mono {
                        eye: Eye::R,
                        state: r_state,
                        timestamp: eyes_frame.frame.timestamp,
                    })
                    .unwrap();
                }
            }
        }
    })
}
