use std::time::SystemTime;

use async_broadcast::Receiver;
use futures::{SinkExt, future::join_all};
use serde_json::Value;
use smallvec::{SmallVec, smallvec};
use tokio::{
    fs::{self, create_dir_all},
    io::AsyncWriteExt,
    net::TcpListener,
    task::JoinHandle,
};
use tokio_stream::StreamExt;
use tokio_util::codec::{Decoder, LinesCodec};

use crate::camera::Frame;

pub fn start_frame_server(l_rx: Receiver<Frame>, r_rx: Receiver<Frame>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let listener = TcpListener::bind("0.0.0.0:7070").await.unwrap();

        loop {
            // Asynchronously wait for an inbound socket.
            let (socket, _) = listener.accept().await.unwrap();

            // And this is where much of the magic of this server happens. We
            // crucially want all clients to make progress concurrently, rather than
            // blocking one on completion of another. To achieve this we use the
            // `tokio::spawn` function to execute the work in the background.
            //
            // Essentially here we're executing a new task to run concurrently,
            // which will allow all of our clients to be processed concurrently.

            let l_rx = l_rx.clone().deactivate();
            let r_rx = r_rx.clone().deactivate();

            tokio::spawn(async move {
                // We're parsing each socket with the `BytesCodec` included in `tokio::codec`.
                let mut framed = LinesCodec::new().framed(socket);

                // We loop while there are messages coming from the Stream `framed`.
                // The stream will return None once the client disconnects.
                while let Some(message) = framed.next().await {
                    match message {
                        Ok(bytes) => {
                            println!("bytes: {bytes:?}");

                            let json: Value = match serde_json::from_str(&bytes) {
                                Ok(parsed) => parsed,
                                Err(e) => {
                                    println!("Failed to parse JSON: {e:?}");
                                    continue;
                                }
                            };

                            let mut cameras: SmallVec<[(Receiver<Frame>, char); 2]> = smallvec![];
                            if json.get("l").is_some() {
                                cameras.push((l_rx.activate_cloned(), 'L'));
                            }
                            if json.get("r").is_some() {
                                cameras.push((r_rx.activate_cloned(), 'R'));
                            }

                            for _ in 0..3 {
                                // Await for a frame from for each eye.
                                let mut frames =
                                    join_all(cameras.iter_mut().map(|(rx, _)| rx.recv())).await;

                                // Add the camera letters back.
                                let mut frames = frames
                                    .iter_mut()
                                    .zip(cameras.iter().map(|(_, letter)| letter));

                                // Check for frame receive errors.
                                if frames.any(|(frame, letter)| {
                                    if let Err(err) = frame {
                                        eprintln!("Failed to get frame {letter} for a save: {err}");
                                        true
                                    } else {
                                        false
                                    }
                                }) {
                                    continue;
                                }

                                // After above, unwrap all of the frames.
                                let frames =
                                    frames.map(|(frame, letter)| (frame.as_mut().unwrap(), letter));

                                let timestamp: chrono::DateTime<chrono::Local> =
                                    SystemTime::now().into();

                                let file_path_str = format!(
                                    "./images/{}.json",
                                    timestamp.format("%Y-%m-%d_%H-%M-%S%.3f")
                                );
                                let file_path = std::path::Path::new(&file_path_str);

                                create_dir_all(file_path.parent().unwrap()).await.unwrap();

                                let mut file = fs::OpenOptions::new()
                                    .create_new(true)
                                    .write(true)
                                    .open(file_path)
                                    .await
                                    .unwrap();
                                file.write_all(bytes.as_bytes()).await.unwrap();

                                for (frame, letter) in frames {
                                    let file_path = format!(
                                        "./images/{}_{}.jpg",
                                        timestamp.format("%Y-%m-%d_%H-%M-%S%.3f"),
                                        letter
                                    );

                                    let mut file = fs::OpenOptions::new()
                                        .create_new(true)
                                        .write(true)
                                        .open(file_path)
                                        .await
                                        .unwrap();

                                    file.write_all(&frame.as_jpeg_bytes()).await.unwrap();
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
