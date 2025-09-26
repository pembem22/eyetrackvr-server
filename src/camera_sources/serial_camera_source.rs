#[derive(Clone, Debug)]
// TODO: implement
pub struct SerialCameraSource {}

// fn connect_serial(&self, tty_path: String) -> JoinHandle<()> {
//     let sender = self.sender.clone();

//     let future = async move {
//         let mut reconnect = false;

//         'connect_loop: loop {
//             if reconnect {
//                 println!("Reconnecting in a sec to {tty_path}");
//                 sleep(Duration::from_secs(1)).await;
//             }
//             reconnect = true;

//             let mut port =
//                 match tokio_serial::new(tty_path.clone(), BAUD_RATE).open_native_async() {
//                     Ok(port) => port,
//                     Err(error) => {
//                         println!("Serial open error: {error:?}");
//                         // return;
//                         continue 'connect_loop;
//                     }
//                 };
//             let mut remaining_bytes = Vec::new();
//             'init: loop {
//                 'find_packet: loop {
//                     remaining_bytes.resize(remaining_bytes.len() + 2048, 0);
//                     let read_position = remaining_bytes.len() - 2048;

//                     match port.read_exact(&mut remaining_bytes[read_position..]).await {
//                         Ok(..) => (),
//                         Err(error) => {
//                             println!("Serial read error: {error:?}");
//                             // TODO: deduplicate
//                             if let Some(raw_err) = error.raw_os_error() {
//                                 if raw_err == 22 {
//                                     continue 'connect_loop;
//                                 }
//                             }
//                             continue 'init;
//                         }
//                     };

//                     for i in 0..remaining_bytes.len() - ETVR_PACKET_HEADER.len() - 2 + 1 {
//                         if remaining_bytes[i..i + ETVR_PACKET_HEADER.len()]
//                             == ETVR_PACKET_HEADER
//                         {
//                             remaining_bytes.drain(0..i);
//                             break 'find_packet;
//                         }
//                     }
//                 }

//                 loop {
//                     let mut buf = [0u8; 6];

//                     let to_copy = std::cmp::min(remaining_bytes.len(), 6);
//                     buf[..to_copy].copy_from_slice(&remaining_bytes[..to_copy]);
//                     remaining_bytes.drain(0..to_copy);
//                     match port.read_exact(&mut buf[to_copy..]).await {
//                         Ok(..) => (),
//                         Err(error) => {
//                             println!("Warning: failed to read exact frame: {error:?}");
//                             // continue 'init;
//                         }
//                     };

//                     if buf[0..4] != ETVR_PACKET_HEADER {
//                         println!("Wrong packet header");
//                         continue 'init;
//                     }
//                     let packet_len = u16::from_le_bytes([buf[4], buf[5]]) as usize;

//                     let mut buf = vec![0; packet_len];

//                     let to_copy = std::cmp::min(remaining_bytes.len(), packet_len);
//                     buf[..to_copy].copy_from_slice(&remaining_bytes[..to_copy]);
//                     remaining_bytes.drain(0..to_copy);
//                     match port.read_exact(&mut buf[to_copy..]).await {
//                         Ok(..) => (),
//                         Err(error) => {
//                             println!("Warning: failed to read exact frame: {error:?}");
//                             // TODO: deduplicate
//                             if let Some(raw_err) = error.raw_os_error() {
//                                 if raw_err == 22 {
//                                     continue 'connect_loop;
//                                 }
//                             }
//                             // continue 'init;
//                         }
//                     };

//                     let mut decoder = image::ImageReader::new(Cursor::new(buf.clone()));
//                     decoder.set_format(image::ImageFormat::Jpeg);

//                     let image = decoder.decode();

//                     if image.is_err() {
//                         println!("Warning: failed to decode image: {:?}", image.unwrap_err());
//                         continue;
//                     }

//                     let image = image.unwrap().as_rgb8().unwrap().to_owned();

//                     let new_frame = Frame {
//                         timestamp: SystemTime::now(),
//                         raw_jpeg_data: Some(buf),
//                         decoded: image,
//                     };

//                     let _ = sender.broadcast_direct(new_frame).await;
//                 }

//                 // println!("{:?} frame! {}", eye, port.bytes_to_read().unwrap());
//             }
//         }
//     };

//     tokio::spawn(future)
// }
