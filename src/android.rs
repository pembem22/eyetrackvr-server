/*
use jni::JNIEnv;
use jni::objects::{JClass, JObject, JString, JValue};

fn enum_and_open_devices(env: &JNIEnv, app_context: JObject) -> jni::errors::Result<()> {
    // 1) fetch Context.USB_SERVICE
    let usb_service_str: JString = env
        .get_static_field(
            "android/content/Context",
            "USB_SERVICE",
            "Ljava/lang/String;",
        )?
        .l()?
        .into();

    // 2) context.getSystemService(Context.USB_SERVICE)
    let usb_mgr = env
        .call_method(
            app_context,
            "getSystemService",
            "(Ljava/lang/String;)Ljava/lang/Object;",
            &[JValue::Object(usb_service_str.into())],
        )?
        .l()?;

    // 3) usbManager.getDeviceList()
    let device_map = env
        .call_method(usb_mgr, "getDeviceList", "()Ljava/util/HashMap;", &[])?
        .l()?;

    // 4) for each UsbDevice in map.values()
    let values = env
        .call_method(device_map, "values", "()Ljava/util/Collection;", &[])?
        .l()?;
    let iter = env
        .call_method(values, "iterator", "()Ljava/util/Iterator;", &[])?
        .l()?;

    while env.call_method(iter, "hasNext", "()Z", &[])?.z()? {
        let dev = env
            .call_method(iter, "next", "()Ljava/lang/Object;", &[])?
            .l()?;

        // check VID/PID
        let vid = env.call_method(dev, "getVendorId", "()I", &[])?.i()?;
        let pid = env.call_method(dev, "getProductId", "()I", &[])?.i()?;

        if vid == 0x05A9 && pid == 0x0680 {
            // 5) openDevice() ➔ UsbDeviceConnection
            let conn = env
                .call_method(
                    usb_mgr,
                    "openDevice",
                    "(Landroid/hardware/usb/UsbDevice;)Landroid/hardware/usb/UsbDeviceConnection;",
                    &[JValue::Object(dev)],
                )
                .unwrap()
                .l()
                .unwrap();

            // 6) int fd = conn.getFileDescriptor()
            let fd = env
                .call_method(conn, "getFileDescriptor", "()I", &[])?
                .i()?;

            // 7) hand off to rusb
            unsafe {
                wrap_fd_into_rusb(fd)?;
            }

            break;
        }
    }

    Ok(())
}
*/

use crate::android_serial_watcher::start_serial_watcher;
use crate::{app::App, camera_server::start_camera_server};
use futures::future::try_join_all;
use tokio::task::JoinHandle;

use crate::window_android::start_ui;

pub fn main() {
    env_logger::builder().format_timestamp(None).init();

    println!("Hello from Android main!");

    std::thread::spawn(|| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            println!("Hello from Tokio runtime!");

            let app = App::new();

            try_join_all(start_android_tasks(&app)).await.unwrap()
        });
    });
    println!("Started Tokio runtime thread");
}

fn start_android_tasks(app: &App) -> Vec<JoinHandle<()>> {
    let mut tasks = Vec::new();

    // HTTP server to mirror the cameras
    tasks.push(start_camera_server(
        app.l_cam_rx.clone(),
        app.r_cam_rx.clone(),
        app.f_cam_rx.clone(),
    ));

    tasks.push(start_serial_watcher(std::collections::HashMap::from([
        ("30:30:F9:33:DD:7C".to_string(), app.l_cam_tx.clone()),
        ("30:30:F9:17:F3:C4".to_string(), app.r_cam_tx.clone()),
        ("DC:DA:0C:18:32:34".to_string(), app.f_cam_tx.clone()),
    ])));

    tasks.push(start_ui(crate::ui::AppRendererContext {
        l_rx: app.l_cam_rx.activate_cloned(),
        r_rx: app.r_cam_rx.activate_cloned(),
        f_rx: app.f_cam_rx.activate_cloned(),
        l_raw_rx: app.l_raw_eye_rx.activate_cloned(),
        r_raw_rx: app.r_raw_eye_rx.activate_cloned(),
        filtered_eyes_rx: app.filtered_eyes_rx.activate_cloned(),
    }));

    // Inference, process the data, output

    #[cfg(feature = "inference")]
    {
        use crate::camera::Eye;
        use crate::data_processing::{filter_eye, merge_eyes};
        use crate::inference::eye_inference;
        use crate::openxr_output::start_openxr_output;

        const THREADS_PER_EYE: usize = 1;

        tasks.push(eye_inference(
            app.l_cam_rx.activate_cloned(),
            THREADS_PER_EYE,
            app.l_raw_eye_tx.clone(),
            Eye::L,
        ));
        tasks.push(eye_inference(
            app.r_cam_rx.activate_cloned(),
            THREADS_PER_EYE,
            app.r_raw_eye_tx.clone(),
            Eye::R,
        ));

        // Filter

        tasks.push(filter_eye(
            app.l_raw_eye_rx.activate_cloned(),
            app.l_filtered_eye_tx.clone(),
        ));
        tasks.push(filter_eye(
            app.r_raw_eye_rx.activate_cloned(),
            app.r_filtered_eye_tx.clone(),
        ));

        // Merge

        tasks.push(merge_eyes(
            app.l_filtered_eye_rx.activate_cloned(),
            app.r_filtered_eye_rx.activate_cloned(),
            app.filtered_eyes_tx.clone(),
        ));

        // OpenXR output
        start_openxr_output(&app.filtered_eyes_rx);
    }

    tasks
}
