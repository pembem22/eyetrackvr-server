use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use async_broadcast::{Receiver, Sender};
use tokio::join;
use tokio::{fs, io::AsyncWriteExt, net::TcpListener, task::JoinHandle};
use tokio_stream::StreamExt;
use tokio_util::codec::{Decoder, LinesCodec};

use crate::inference::start_onnx;
use crate::Frame;
use crate::{camera_texture::CameraTexture, ui, Camera, Eye};

pub(crate) struct App {
    l_camera: Camera,
    r_camera: Camera,
}

impl App {
    pub fn new(l_sender: Sender<Frame>, r_sender: Sender<Frame>) -> App {
        let l_camera = Camera::new(Eye::L, l_sender);
        let r_camera = Camera::new(Eye::R, r_sender);

        App { l_camera, r_camera }
    }

    pub fn start_cameras(
        &mut self,
        l_tty_path: String,
        r_tty_path: String,
    ) -> tokio_serial::Result<(JoinHandle<()>, JoinHandle<()>)> {
        Ok((
            self.l_camera.start(l_tty_path)?,
            self.r_camera.start(r_tty_path)?,
        ))
    }

    pub fn start_ui(&mut self, l_rx: Receiver<Frame>, r_rx: Receiver<Frame>) -> JoinHandle<()> {
        tokio::task::spawn_blocking(|| {
            let mut ui = ui::UI::new();

            let l_texture = CameraTexture::new(&mut ui);
            let r_texture = CameraTexture::new(&mut ui);

            let l_rx = Arc::new(Mutex::new(l_rx));
            let r_rx = Arc::new(Mutex::new(r_rx));

            ui.run(move |imgui, queue, renderer| {
                let mut l_rx = l_rx.lock().unwrap();
                let mut r_rx = r_rx.lock().unwrap();

                l_texture.update_texture(&mut l_rx, queue, renderer);
                r_texture.update_texture(&mut r_rx, queue, renderer);

                imgui.window("Hello!").build(move || {
                    l_texture.build(imgui);
                    imgui.same_line();
                    r_texture.build(imgui);
                });
            });
        })
    }

    pub fn start_server(&mut self, l_rx: Receiver<Frame>, r_rx: Receiver<Frame>) -> JoinHandle<()> {
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
                                println!("bytes: {:?}", bytes);

                                let mut l_rx = l_rx.activate_cloned();
                                let mut r_rx = r_rx.activate_cloned();

                                // Await for a frame from for each eye.
                                let (mut l_frame, mut r_frame) = join!(l_rx.recv(), r_rx.recv());

                                // Grab new frames if we got some new ones while awaiting above or skip overflow error.
                                if let Ok(frame) = l_rx.try_recv() {
                                    l_frame = Ok(frame)
                                }
                                if let Ok(frame) = r_rx.try_recv() {
                                    r_frame = Ok(frame)
                                }

                                let (l_frame, r_frame) = match (l_frame, r_frame) {
                                    (Ok(l_frame), Ok(r_frame)) => (l_frame, r_frame),
                                    (_, _) => {
                                        println!("Failed to get frames for a save");
                                        continue;
                                    }
                                };

                                let timestamp: chrono::DateTime<chrono::Local> =
                                    SystemTime::now().into();

                                let file_path = format!(
                                    "./images/{}.json",
                                    timestamp.format("%Y-%m-%d_%H-%M-%S%.3f")
                                );

                                let mut file = fs::OpenOptions::new()
                                    .create(true)
                                    .write(true)
                                    .open(file_path.clone())
                                    .await
                                    .unwrap();

                                file.write_all(bytes.as_bytes()).await.unwrap();

                                {
                                    let file_path = format!(
                                        "./images/{}_L.jpg",
                                        timestamp.format("%Y-%m-%d_%H-%M-%S%.3f")
                                    );

                                    let mut file = fs::OpenOptions::new()
                                        .create(true)
                                        .write(true)
                                        .open(file_path.clone())
                                        .await
                                        .unwrap();

                                    file.write_all(&l_frame.raw_data).await.unwrap();
                                }

                                {
                                    let file_path = format!(
                                        "./images/{}_R.jpg",
                                        timestamp.format("%Y-%m-%d_%H-%M-%S%.3f")
                                    );

                                    let mut file = fs::OpenOptions::new()
                                        .create(true)
                                        .write(true)
                                        .open(file_path.clone())
                                        .await
                                        .unwrap();

                                    file.write_all(&r_frame.raw_data).await.unwrap();
                                }
                            }
                            Err(err) => println!("Socket closed with error: {:?}", err),
                        }
                    }
                    println!("Socket received FIN packet and closed connection");
                });
            }
        })
    }

    pub fn start_inference(
        &mut self,
        osc_out_address: String,
        model_path: String,
        threads_per_eye: usize,
        l_rx: Receiver<Frame>,
        r_rx: Receiver<Frame>,
    ) -> JoinHandle<()> {
        let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
        sock.connect(osc_out_address).unwrap();
        println!("OSC connected");

        start_onnx(l_rx, r_rx, sock, model_path, threads_per_eye).unwrap()
    }
}
