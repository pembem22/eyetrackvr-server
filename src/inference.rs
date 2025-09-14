use async_broadcast::{Receiver, RecvError, Sender};
use image::{DynamicImage, GenericImageView};
use ort::{
    session::{Session, builder::GraphOptimizationLevel},
    value::TensorRef,
};
use tokio::task::JoinHandle;

use crate::structs::{EyeGazeState, EyesGazeState};
use crate::{
    camera::{Eye, Frame},
    structs::EyesFrame,
};

pub const FRAME_CROP_X: u32 = 30;
pub const FRAME_CROP_Y: u32 = 30;
pub const FRAME_CROP_W: u32 = 180;
pub const FRAME_CROP_H: u32 = 60;
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
            let eyes_frame = match rx.recv_blocking() {
                Ok(eyes_frame) => Some(eyes_frame),
                Err(e) => {
                    match e {
                        RecvError::Overflowed(skipped) => {
                            println!("Skipped {skipped} frames")
                        }
                        RecvError::Closed => {
                            println!("Channel closed");
                            break;
                        }
                    };
                    None
                }
            };

            let eyes_frame = match eyes_frame {
                Some(eyes_frame) => eyes_frame,
                None => continue,
            };

            let mut run_eye_inference =
                |frame_view: image::SubImage<&image::ImageBuffer<image::Rgb<u8>, Vec<u8>>>,
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
                        timestamp: eyes_frame.frame.timestamp,
                    }
                };

            let l_eye_result = run_eye_inference(eyes_frame.get_left_view().unwrap(), true);
            let r_eye_result: EyeGazeState =
                run_eye_inference(eyes_frame.get_right_view().unwrap(), false);

            // println!("{:#?}", EyesGazeState {
            //     l: l_eye_result,
            //     r: r_eye_result,
            // });

            tx.broadcast_blocking(EyesGazeState {
                l: l_eye_result,
                r: r_eye_result,
            })
            .unwrap();
        }
    })
}
