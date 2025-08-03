use jni::{
    objects::{JClass, JString, JValue},
    JNIEnv, JavaVM,
};
use jni::sys::{self, JNI_OK, JNI_GetCreatedJavaVMs};
use std::collections::HashMap;
use std::ptr;
use std::sync::{LazyLock, Mutex};

// Global storage for callbacks
static CALLBACKS: LazyLock<Mutex<HashMap<String, Box<dyn Fn(String) + Send + Sync>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Gets the JavaVM instance by querying the created VMs from JNI.
/// In a production implementation you’d obtain the JavaVM pointer more directly (e.g. during JNI_OnLoad).
fn get_java_vm() -> Option<JavaVM> {
    unsafe {
        // Allocate a one-element array to hold the raw JavaVM pointer.
        let mut vm_buf: [*mut sys::JavaVM; 1] = [ptr::null_mut()];
        let mut vm_count: i32 = 0;
        if JNI_GetCreatedJavaVMs(vm_buf.as_mut_ptr(), 1, &mut vm_count) == JNI_OK as i32
            && vm_count > 0
        {
            let raw_vm = vm_buf[0];
            if !raw_vm.is_null() {
                // Convert the raw pointer into a jni::JavaVM using the `from` associated function.
                JavaVM::from(raw_vm).ok()
            } else {
                None
            }
        } else {
            None
        }
    }
}

/// Registers a callback for a given ID.
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

/// Evaluate JavaScript on Android by invoking the `evalJs` static method of the
/// `io.github.memkit.RustBridge` Java class.
pub async fn eval_js(js_code: &str) -> Result<(), String> {
    // Get the JavaVM.
    let vm = get_java_vm().ok_or("Failed to get JavaVM")?;
    // Attach the current thread to the JVM.
    let mut env = vm.attach_current_thread()
        .map_err(|e| format!("Failed to attach to JVM: {:?}", e))?;
    
    // Find the RustBridge class.
    let class_name = "io/github/memkit/RustBridge";
    let class = env.find_class(class_name)
        .map_err(|e| format!("Failed to find class {}: {:?}", class_name, e))?;
    
    // Call the static method "evalJs" with a Java string argument.
    env.call_static_method(
        class,
        "evalJs",
        "(Ljava/lang/String;)V",
        &[JValue::Object(&env.new_string(js_code)
            .map_err(|e| format!("Failed to create Java string: {:?}", e))?
            .into())]
    ).map_err(|e| format!("Failed to call evalJs: {:?}", e))?;
    
    // Check for any thrown exceptions.
    if env.exception_check().map_err(|e| format!("Failed to check for exceptions: {:?}", e))? {
        env.exception_describe()
            .map_err(|e| format!("Failed to describe exception: {:?}", e))?;
        env.exception_clear()
            .map_err(|e| format!("Failed to clear exception: {:?}", e))?;
        return Err("JavaScript evaluation threw an exception".to_string());
    }
    
    Ok(())
}

/// Send data to Java/Kotlin by calling the static method `onMessageFromRust` on
/// the `io.github.memkit.RustBridge` Java class.
pub async fn send_to_java(message: String) -> Result<(), String> {
    // Get the JavaVM.
    let vm = get_java_vm().ok_or("Failed to get JavaVM")?;
    // Attach the current thread to the JVM.
    let mut env = vm.attach_current_thread()
        .map_err(|e| format!("Failed to attach to JVM: {:?}", e))?;
    
    // Find the RustBridge class.
    let class_name = "io/github/memkit/RustBridge";
    let class = env.find_class(class_name)
        .map_err(|e| format!("Failed to find class {}: {:?}", class_name, e))?;
    
    // Call the static method "onMessageFromRust" with a Java string argument.
    env.call_static_method(
        class,
        "onMessageFromRust",
        "(Ljava/lang/String;)V",
        &[JValue::Object(&env.new_string(&message)
            .map_err(|e| format!("Failed to create Java string: {:?}", e))?
            .into())]
    ).map_err(|e| format!("Failed to call onMessageFromRust: {:?}", e))?;
    
    // Check for any thrown exceptions.
    if env.exception_check().map_err(|e| format!("Failed to check for exceptions: {:?}", e))? {
        env.exception_describe()
            .map_err(|e| format!("Failed to describe exception: {:?}", e))?;
        env.exception_clear()
            .map_err(|e| format!("Failed to clear exception: {:?}", e))?;
        return Err("Sending message to Java threw an exception".to_string());
    }
    
    Ok(())
}

/// JNI function called from Java/Kotlin when a message is received.
/// This function converts the incoming Java strings to Rust strings and then
/// calls the registered callback for the given callback ID.
#[no_mangle]
pub extern "system" fn Java_io_github_memkit_RustBridge_onMessageFromJava(
    mut env: JNIEnv,
    _class: JClass,
    callback_id: JString,
    json_data: JString,
) {
    // Convert the Java strings to Rust strings.
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
    
    // Look up and call the callback.
    let callbacks = CALLBACKS.lock().unwrap();
    if let Some(callback) = callbacks.get(&callback_id_str) {
        callback(json_data_str);
    }
}