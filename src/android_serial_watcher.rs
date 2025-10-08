use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use android_usbser::{CdcSerial, usb};
use hex_literal::hex;
use log::{debug, error, info, warn};
use pollster::FutureExt;
use tokio::task::JoinHandle;
use tokio_serial::SerialPort;
use tokio_stream::StreamExt;

use crate::camera::Frame;
use crate::camera_dispatcher::CameraDispatcher;
use crate::camera_sources::FpsCounter;

const BAUD_RATE: u32 = 3000000;

const ETVR_PACKET_HEADER: [u8; 4] = hex!("FF A0 FF A1");

const USB_SERIAL_MAX_PACKET_SIZE: usize = 64;

struct SerialByteStream {
    serial: CdcSerial,
    buf: [u8; USB_SERIAL_MAX_PACKET_SIZE],
    pos: usize,
    bytes_read: usize,
}

impl SerialByteStream {
    fn new(serial: CdcSerial) -> Self {
        SerialByteStream {
            serial,
            buf: [0u8; 64],
            pos: 0,
            bytes_read: 0,
        }
    }
}

impl Iterator for SerialByteStream {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.bytes_read {
            match self.serial.read(&mut self.buf) {
                Ok(bytes_read) => {
                    self.bytes_read = bytes_read;
                    self.pos = 0;
                }
                Err(_) => return None,
            }
        }
        if self.pos < self.bytes_read {
            let byte = self.buf[self.pos];
            self.pos += 1;
            Some(byte)
        } else {
            None
        }
    }
}

pub fn start_serial_watcher(
    mac_to_sender: HashMap<String, Box<dyn CameraDispatcher>>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mac_to_sender = Arc::new(Mutex::new(mac_to_sender));

        let mut devices_stream = futures::stream::iter(usb::list_devices().unwrap())
            .map(|d| usb::HotplugEvent::Connected(d))
            .merge(usb::watch_devices().unwrap());

        loop {
            let mac_to_sender = mac_to_sender.clone();
            match devices_stream.next().await {
                None => continue,
                Some(usb::HotplugEvent::Connected(dev)) => {
                    debug!("New USB device: {dev:#?}");

                    let has_permission = match dev.has_permission() {
                        Ok(has_permission) => has_permission,
                        Err(err) => {
                            error!("Failed to check if have USB permission: {err:?}");
                            continue;
                        }
                    };

                    let dev_info = if has_permission {
                        info!("Already have device permission");
                        dev
                    } else {
                        info!("Performing permission request");
                        match dev.request_permission() {
                            Ok(perm_req) => match perm_req {
                                Some(perm_req) => {
                                    info!("Waiting for a permission request response...");
                                    if !perm_req.await {
                                        info!("Permission denied.");
                                        continue;
                                    }
                                    info!("Permission allowed");
                                    dev
                                }
                                None => {
                                    error!("Falied to request permission");
                                    continue;
                                }
                            },
                            Err(err) => {
                                error!("Failed to request USB permission: {err:?}");
                                continue;
                            }
                        }
                    };

                    // let Ok(conn) = dev_info.open_device() else {
                    //     println!("Unexpected: failed to open the device.");
                    //     continue;
                    // };
                    // println!("Opened {dev_info:?}");

                    // Need to do this because of the bug described in `DeviceInfo::serial_number`.
                    let Some(serial_num) = usb::list_devices()
                        .unwrap()
                        .iter()
                        .find(|d| d.path_name() == dev_info.path_name())
                        .unwrap()
                        .serial_number()
                        .clone()
                    else {
                        info!("Serial is None, skipping");
                        continue;
                    };

                    info!("Serial {serial_num:?}");

                    let Some(dispatcher) = mac_to_sender.lock().unwrap().remove(&serial_num) else {
                        info!("Serial {serial_num:?} is not recognized");
                        continue;
                    };

                    info!("Recognized serial {serial_num:?}");

                    tokio::task::spawn_blocking(move || {
                        info!("Started blocking task for serial {serial_num}");
                        let mut serial =
                            CdcSerial::build(&dev_info, Duration::from_millis(300)).unwrap();
                        info!("Opened, setting config...");
                        serial.set_baud_rate(BAUD_RATE).unwrap();
                        serial.set_parity(tokio_serial::Parity::None).unwrap();
                        serial.set_data_bits(tokio_serial::DataBits::Eight).unwrap();
                        serial.set_stop_bits(tokio_serial::StopBits::One).unwrap();
                        info!("Configuration set.");

                        let mut stream = SerialByteStream::new(serial);
                        let mut last_bytes = [0u8; 6];
                        let mut collecting = false;
                        let mut image_data = Vec::with_capacity(8192);
                        let mut image_size = 0;

                        let mut fps = FpsCounter::new();

                        while let Some(byte) = stream.next() {
                            // Shift the buffer and add the new byte.
                            // TODO: use a circular buffer. Does it even matter?
                            last_bytes.copy_within(1.., 0);
                            last_bytes[last_bytes.len() - 1] = byte;

                            if collecting && image_data.len() == image_size {
                                // println!("{serial_num}:");
                                fps.update_fps();

                                // Process the collected image.
                                let mut decoder = image::ImageReader::new(Cursor::new(&image_data));
                                decoder.set_format(image::ImageFormat::Jpeg);

                                if let Ok(image) = decoder.decode() {
                                    let image = image.as_rgb8().unwrap().to_owned();
                                    let new_frame = Frame {
                                        timestamp: SystemTime::now(),
                                        raw_jpeg_data: Some(image_data.clone()),
                                        decoded: image,
                                    };
                                    dispatcher.dispatch(new_frame).block_on();
                                } else {
                                    warn!("failed to decode image");
                                }

                                collecting = false;
                            } else if last_bytes[0..4] == ETVR_PACKET_HEADER {
                                image_size =
                                    u16::from_le_bytes([last_bytes[4], last_bytes[5]]) as usize;

                                // Start collecting the next image.
                                image_data.clear();
                                collecting = true;
                            } else if collecting {
                                image_data.push(byte);
                            }
                        }

                        mac_to_sender.lock().unwrap().insert(serial_num, dispatcher);

                        info!("Serial stream ended");
                    });
                }
                Some(usb::HotplugEvent::Disconnected(dev)) => {
                    info!("Device disconnected ({}).", dev.path_name());
                    continue;
                }
            }
        }
    })
}
