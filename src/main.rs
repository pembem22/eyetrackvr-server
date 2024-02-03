use std::{cmp::min, io::Cursor, path::Path, sync::Arc, time::SystemTime};

use imgui_wgpu::{Texture, TextureConfig};
use tokio::{
    fs,
    io::{AsyncReadExt, AsyncWriteExt},
    join,
    net::TcpListener,
    sync::Mutex,
    task::JoinHandle,
};
use tokio_serial::SerialPortBuilderExt;

use tokio_stream::StreamExt;
use tokio_util::codec::{BytesCodec, Decoder};

mod camera;
mod camera_texture;
mod ui;

use crate::{camera::*, camera_texture::CameraTexture};

#[tokio::main]
async fn main() -> tokio_serial::Result<()> {
    let mut l_camera = Camera::new(Eye::L);
    let mut r_camera = Camera::new(Eye::R);

    l_camera.start("COM3".to_string())?;
    r_camera.start("COM4".to_string())?;

    let ui_l_frame = l_camera.frame.clone();
    let ui_r_frame = r_camera.frame.clone();
    let ui_task = tokio::task::spawn_blocking(|| {
        let mut ui = ui::UI::new();

        let l_texture = CameraTexture::new(&mut ui);
        let r_texture = CameraTexture::new(&mut ui);

        ui.run(move |imgui, queue, renderer| {
            l_texture.update_texture(&ui_l_frame.blocking_lock(), queue, renderer);
            r_texture.update_texture(&ui_r_frame.blocking_lock(), queue, renderer);

            imgui.window("Hello!").build(move || {
                l_texture.build(imgui);
                imgui.same_line();
                r_texture.build(imgui);
            });
        });
    });

    let listener = TcpListener::bind("0.0.0.0:7070").await?;

    loop {
        // Asynchronously wait for an inbound socket.
        let (socket, _) = listener.accept().await?;

        // And this is where much of the magic of this server happens. We
        // crucially want all clients to make progress concurrently, rather than
        // blocking one on completion of another. To achieve this we use the
        // `tokio::spawn` function to execute the work in the background.
        //
        // Essentially here we're executing a new task to run concurrently,
        // which will allow all of our clients to be processed concurrently.
        let thread_l_frame = l_camera.frame.clone();
        let thread_r_frame = r_camera.frame.clone();
        tokio::spawn(async move {
            // We're parsing each socket with the `BytesCodec` included in `tokio::codec`.
            let mut framed = BytesCodec::new().framed(socket);

            // We loop while there are messages coming from the Stream `framed`.
            // The stream will return None once the client disconnects.
            while let Some(message) = framed.next().await {
                match message {
                    Ok(bytes) => {
                        println!("bytes: {:?}", bytes);

                        let l_frame = thread_l_frame.lock().await;
                        let r_frame = thread_r_frame.lock().await;

                        let timestamp: chrono::DateTime<chrono::Local> = SystemTime::now().into();

                        let file_path =
                            format!("./images/{}.json", timestamp.format("%Y-%m-%d_%H-%M-%S%.3f"));

                        let mut file = fs::OpenOptions::new()
                            .create(true)
                            .write(true)
                            .open(file_path.clone())
                            .await
                            .unwrap();

                        file.write_all(&bytes).await.unwrap();

                        {
                            let file_path =
                                format!("./images/{}_L.jpg", timestamp.format("%Y-%m-%d_%H-%M-%S%.3f"));

                            let mut file = fs::OpenOptions::new()
                                .create(true)
                                .write(true)
                                .open(file_path.clone())
                                .await
                                .unwrap();

                            file.write_all(&l_frame.data).await.unwrap();
                        }

                        {
                            let file_path =
                                format!("./images/{}_R.jpg", timestamp.format("%Y-%m-%d_%H-%M-%S%.3f"));

                            let mut file = fs::OpenOptions::new()
                                .create(true)
                                .write(true)
                                .open(file_path.clone())
                                .await
                                .unwrap();

                            file.write_all(&r_frame.data).await.unwrap();
                        }
                    }
                    Err(err) => println!("Socket closed with error: {:?}", err),
                }
            }
            println!("Socket received FIN packet and closed connection");
        });
    }

    join!(l_camera.task.unwrap(), r_camera.task.unwrap(), ui_task);
    Ok(())
}
