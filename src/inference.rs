use std::time::SystemTime;

use async_broadcast::{Receiver, RecvError, Sender};
use image::{DynamicImage, GenericImageView};
use ort::session::{builder::GraphOptimizationLevel, Session};
use tokio::runtime::Handle;
use tokio::task::JoinHandle;

use crate::{Eye, Frame};

#[derive(Copy, Clone, Debug)]
pub struct EyeState {
    pub pitch: f32,
    pub yaw: f32,
    pub openness: f32,
    pub timestamp: SystemTime,
}

impl Default for EyeState {
    fn default() -> Self {
        Self {
            pitch: Default::default(),
            yaw: Default::default(),
            openness: Default::default(),
            timestamp: SystemTime::UNIX_EPOCH,
        }
    }
}

pub fn eye_inference(
    mut rx: Receiver<Frame>,
    model_path: &str,
    threads: usize,
    tx: Sender<EyeState>,
    eye: Eye,
) -> JoinHandle<()> {
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
            let output = output.flatten();

            let _ = handle.block_on(async {
                tx.broadcast_direct(EyeState {
                    pitch: output[0],
                    yaw: output[1] * if eye == Eye::R { -1.0 } else { 1.0 },
                    openness: output[2],
                    timestamp: frame.timestamp,
                })
                .await
            });
        }
    })
}
