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

pub fn main() {
    println!("Hello from Android main!");
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        println!("hello");
    });
    println!("Started Tokio runtime");
}

fn setup_android_tasks() {}
