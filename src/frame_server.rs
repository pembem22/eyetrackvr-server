use std::{io::Cursor, time::SystemTime};

use async_broadcast::{InactiveReceiver, RecvError};
use futures::SinkExt;
use image::codecs::png::PngEncoder;
use smallvec::{SmallVec, smallvec};
use tokio::{
    fs::{self, create_dir_all},
    io::AsyncWriteExt,
    net::TcpListener,
    task::JoinHandle,
};
use tokio_stream::StreamExt;
use tokio_util::codec::{Decoder, LinesCodec};

use crate::structs::{EyesFrame, EyesFrameType};

const DATETIME_FORMAT: &str = "%Y-%m-%d_%H-%M-%S%.3f";
const FRAMES_PER_CAPTURE: u32 = 3;

pub fn start_frame_server(rx: InactiveReceiver<EyesFrame>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let listener = TcpListener::bind("0.0.0.0:7070").await.unwrap();

        loop {
            let (socket, _) = listener.accept().await.unwrap();

            let rx = rx.clone();

            tokio::spawn(async move {
                let mut framed = LinesCodec::new().framed(socket);

                let mut rx = rx.activate();

                while let Some(message) = framed.next().await {
                    match message {
                        Ok(bytes) => {
                            println!("bytes: {bytes:?}");

                            let json: serde_json::Value = match serde_json::from_str(&bytes) {
                                Ok(parsed) => parsed,
                                Err(e) => {
                                    println!("Failed to parse JSON: {e:?}");
                                    continue;
                                }
                            };

                            let need_l_view = json.get("l").is_some();
                            let need_r_view = json.get("r").is_some();

                            for _ in 0..FRAMES_PER_CAPTURE {
                                let frame = loop {
                                    match rx.recv().await {
                                        Ok(frame) => break Some(frame),
                                        Err(e) => {
                                            match e {
                                                RecvError::Overflowed(skipped) => {
                                                    println!("Skipped {skipped} frames");
                                                    continue;
                                                }
                                                RecvError::Closed => {
                                                    println!("Channel closed");
                                                    break None;
                                                }
                                            };
                                        }
                                    }
                                };

                                let Some(frame) = frame else { return };

                                if frame.frame_type != EyesFrameType::Both {
                                    eprintln!("Saving mono frames not supported!");
                                    break;
                                }

                                let mut views: SmallVec<
                                    [(
                                        image::SubImage<
                                            &image::ImageBuffer<image::Rgb<u8>, Vec<u8>>,
                                        >,
                                        char,
                                    ); 2],
                                > = smallvec![];

                                if need_l_view {
                                    views.push((frame.get_left_view().unwrap(), 'L'));
                                }
                                if need_r_view {
                                    views.push((frame.get_right_view().unwrap(), 'R'));
                                }

                                let timestamp: chrono::DateTime<chrono::Local> =
                                    SystemTime::now().into();

                                let formatted_datetime = timestamp.format(DATETIME_FORMAT);

                                let file_path_str = format!("./images/{}.json", formatted_datetime);
                                let file_path = std::path::Path::new(&file_path_str);

                                create_dir_all(file_path.parent().unwrap()).await.unwrap();

                                let mut file = fs::OpenOptions::new()
                                    .create_new(true)
                                    .write(true)
                                    .open(file_path)
                                    .await
                                    .unwrap();
                                file.write_all(bytes.as_bytes()).await.unwrap();

                                for (frame, letter) in views {
                                    let file_path =
                                        format!("./images/{}_{}.jpg", formatted_datetime, letter);

                                    let mut file = fs::OpenOptions::new()
                                        .create_new(true)
                                        .write(true)
                                        .open(file_path)
                                        .await
                                        .unwrap();

                                    let vec = Vec::with_capacity(8192);
                                    let mut cursor = Cursor::new(vec);

                                    frame
                                        .to_image()
                                        .write_with_encoder(PngEncoder::new(&mut cursor))
                                        .unwrap();

                                    file.write_all(&cursor.into_inner()).await.unwrap();
                                }
                            }

                            let _ = framed.send("k").await;
                        }
                        Err(err) => println!("Socket closed with error: {err:?}"),
                    }
                }
                println!("Socket received FIN packet and closed connection");
            });
        }
    })
}
