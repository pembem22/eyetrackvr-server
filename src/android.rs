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

use futures::future::try_join_all;
use tokio::task::JoinHandle;

use crate::{app::App, camera_server::start_camera_server};

pub fn main() {
    println!("Hello from Android main!");
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        println!("Hello from Tokio runtime!");

        let app = App::new();

        try_join_all(start_android_tasks(&app)).await.unwrap()
    });
    println!("Started Tokio runtime");
}

fn start_android_tasks(app: &App) -> Vec<JoinHandle<()>> {
    let mut tasks = Vec::new();

    let (l_camera, r_camera, f_camera) = app.start_cameras(
        "http://localhost:8080/30:30:F9:33:DD:7C".to_string(),
        "http://localhost:8080/30:30:F9:17:F3:C4".to_string(),
        "http://localhost:8080/DC:DA:0C:18:32:34".to_string(),
    );
    tasks.push(l_camera);
    tasks.push(r_camera);
    tasks.push(f_camera);

    // HTTP server to mirror the cameras
    tasks.push(start_camera_server(
        app.l_cam_rx.activate_cloned(),
        app.f_cam_rx.activate_cloned(),
    ));

    tasks
}
