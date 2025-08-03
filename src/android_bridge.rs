use jni::sys;
use jni::JavaVM;
use std::sync::Once;

// Global static to hold the JavaVM pointer
static mut GLOBAL_JAVA_VM: *mut sys::JavaVM = std::ptr::null_mut();
static INIT: Once = Once::new();

// This function should be called from JNI_OnLoad in your native library.
#[no_mangle]
pub unsafe extern "C" fn JNI_OnLoad(
    vm: *mut sys::JavaVM,
    _reserved: *mut std::ffi::c_void,
) -> i32 {
    INIT.call_once(|| {
        GLOBAL_JAVA_VM = vm;
    });
    // Return the JNI version you support (e.g., JNI_VERSION_1_6)
    sys::JNI_VERSION_1_8
}

// Now, use conditional compilation to avoid using JNI_GetCreatedJavaVMs on Android.
#[cfg(target_os = "android")]
fn get_java_vm() -> Option<JavaVM> {
    unsafe {
        if GLOBAL_JAVA_VM.is_null() {
            None
        } else {
            // Safety: This assumes that the JavaVM pointer remains valid.
            JavaVM::from_raw(GLOBAL_JAVA_VM as *mut sys::JavaVM).ok()
        }
    }
}

// For non-Android platforms, you may still use the original approach.
#[cfg(not(target_os = "android"))]
fn get_java_vm() -> Option<JavaVM> {
    use std::ptr;
    unsafe {
        let mut vm_buf: [*mut sys::JavaVM; 1] = [ptr::null_mut()];
        let mut vm_count: i32 = 0;
        if sys::JNI_GetCreatedJavaVMs(vm_buf.as_mut_ptr(), 1, &mut vm_count)
            == sys::JNI_OK as i32
            && vm_count > 0
        {
            let raw_vm = vm_buf[0];
            if !raw_vm.is_null() {
                JavaVM::from_raw(raw_vm as *mut sys::JavaVM).ok()
            } else {
                None
            }
        } else {
            None
        }
    }
}

/// Registers a callback for the given ID.
pub fn register_callback<F>(id: String, callback: F)
where
    F: Fn(String) + Send + Sync + 'static,
{
    let mut callbacks = CALLBACKS.lock().unwrap();
    callbacks.insert(id, Box::new(callback));
}

/// Unregisters a callback.
pub fn unregister_callback(id: &str) {
    let mut callbacks = CALLBACKS.lock().unwrap();
    callbacks.remove(id);
}

/// Evaluates JavaScript on Android by calling the static method `evalJs` on
/// the Java class "io.github.memkit.RustBridge".
pub async fn eval_js(js_code: &str) -> Result<(), String> {
    // Retrieve the JavaVM.
    let vm = get_java_vm().ok_or("Failed to get JavaVM")?;
    // Attach the current thread to the JVM.
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| format!("Failed to attach to JVM: {:?}", e))?;
    // Find the class "io/github/memkit/RustBridge".
    let class_name = "io/github/memkit/RustBridge";
    let class = env
        .find_class(class_name)
        .map_err(|e| format!("Failed to find class {}: {:?}", class_name, e))?;
    // Create a Java string from js_code.
    let js_string = env
        .new_string(js_code)
        .map_err(|e| format!("Failed to create Java string: {:?}", e))?;
    // Convert the JString into a JObject.
    let js_obj: JObject = JObject::from(js_string);
    // NOTE: JValue::Object requires a reference to the JObject.
    let args = [JValue::Object(&js_obj)];
    // Call the static method "evalJs".
    env.call_static_method(class, "evalJs", "(Ljava/lang/String;)V", &args)
        .map_err(|e| format!("Failed to call evalJs: {:?}", e))?;
    // Check for exceptions.
    if env
        .exception_check()
        .map_err(|e| format!("Failed to check for exceptions: {:?}", e))?
    {
        env.exception_describe()
            .map_err(|e| format!("Failed to describe exception: {:?}", e))?;
        env.exception_clear()
            .map_err(|e| format!("Failed to clear exception: {:?}", e))?;
        return Err("JavaScript evaluation threw an exception".to_string());
    }
    Ok(())
}

/// Sends data to Java/Kotlin by calling the static method `onMessageFromRust` on
/// the Java class "io.github.memkit.RustBridge".
pub async fn send_to_java(message: String) -> Result<(), String> {
    let vm = get_java_vm().ok_or("Failed to get JavaVM")?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| format!("Failed to attach to JVM: {:?}", e))?;
    let class_name = "io/github/memkit/RustBridge";
    let class = env
        .find_class(class_name)
        .map_err(|e| format!("Failed to find class {}: {:?}", class_name, e))?;
    let msg_string = env
        .new_string(&message)
        .map_err(|e| format!("Failed to create Java string: {:?}", e))?;
    let msg_obj: JObject = JObject::from(msg_string);
    let args = [JValue::Object(&msg_obj)];
    env.call_static_method(
        class,
        "onMessageFromRust",
        "(Ljava/lang/String;)V",
        &args,
    )
    .map_err(|e| format!("Failed to call onMessageFromRust: {:?}", e))?;
    if env
        .exception_check()
        .map_err(|e| format!("Failed to check for exceptions: {:?}", e))?
    {
        env.exception_describe()
            .map_err(|e| format!("Failed to describe exception: {:?}", e))?;
        env.exception_clear()
            .map_err(|e| format!("Failed to clear exception: {:?}", e))?;
        return Err("Sending message to Java threw an exception".to_string());
    }
    Ok(())
}

/// This JNI function is called from Java/Kotlin when a message is received.
/// It converts the incoming Java strings to Rust strings and invokes the
/// registered callback for the provided callback ID.
#[no_mangle]
pub extern "system" fn Java_io_github_memkit_RustBridge_onMessageFromJava(
    mut env: JNIEnv,
    _class: JClass,
    callback_id: JString,
    json_data: JString,
) {
    let callback_id_rust = match env.get_string(&callback_id) {
        Ok(s) => s,
        Err(_) => return,
    };
    let callback_id_str = match callback_id_rust.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return,
    };
    let json_data_rust = match env.get_string(&json_data) {
        Ok(s) => s,
        Err(_) => return,
    };
    let json_data_str = match json_data_rust.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return,
    };
    let callbacks = CALLBACKS.lock().unwrap();
    if let Some(callback) = callbacks.get(&callback_id_str) {
        callback(json_data_str);
    }
}