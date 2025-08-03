use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Mutex;

// Only include JNI-specific code when targeting Android
#[cfg(target_os = "android")]
use jni::JNIEnv;
#[cfg(target_os = "android")]
use jni::objects::{JClass, JString};
#[cfg(target_os = "android")]
use jni::sys::jstring;

static CALLBACKS: Lazy<Mutex<HashMap<String, Box<dyn Fn(String) + Send + Sync>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn register_callback<F: Fn(String) + Send + Sync + 'static>(id: String, cb: F) {
    CALLBACKS.lock().unwrap().insert(id, Box::new(cb));
}

pub fn unregister_callback(id: &str) {
    CALLBACKS.lock().unwrap().remove(id);
}

/// Send a message to Java/Kotlin via JNI
pub async fn send_to_java(message: String) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        // Get the JNI environment
        let env = match jni::AttachGuard::new() {
            Ok(guard) => guard,
            Err(e) => {
                return Err(format!("Failed to attach to JNI: {}", e));
            }
        };
        
        // Convert the message to a Java string
        let jmessage = match env.new_string(&message) {
            Ok(s) => s.into_inner(),
            Err(e) => {
                return Err(format!("Failed to create Java string: {}", e));
            }
        };
        
        // Call the static method in MainActivity
        unsafe {
            Java_dev_dioxus_main_MainActivity_sendJsBridgeMessage(
                *env,
                JClass::from(std::ptr::null_mut()), // Static method, so class is null
                jmessage,
            );
        }
        
        Ok(())
    }
    
    #[cfg(not(target_os = "android"))]
    {
        Err("Android bridge not available on this platform".to_string())
    }
}

/// Evaluate JavaScript on Android
pub async fn eval_js(js_code: &str) -> Result<(), String> {
    // Create a message for JS evaluation
    let message = format!("{{\"type\":\"eval\",\"code\":\"{}\"}}", js_code);
    send_to_java(message).await
}

/// Call this from Java/Kotlin via JNI, passing the callback_id and the JSON string.
#[no_mangle]
pub extern "C" fn rust_js_bridge_callback(
    callback_id: *const libc::c_char,
    json: *const libc::c_char,
) {
    use std::ffi::CStr;
    let callback_id = unsafe { CStr::from_ptr(callback_id) }
        .to_string_lossy()
        .to_string();
    let json = unsafe { CStr::from_ptr(json) }
        .to_string_lossy()
        .to_string();
    if let Some(cb) = CALLBACKS.lock().unwrap().get(&callback_id) {
        cb(json);
    }
}

// Declare the JNI function for sending messages to Java
#[cfg(target_os = "android")]
#[allow(non_snake_case)]
extern "C" {
    fn Java_dev_dioxus_main_MainActivity_sendJsBridgeMessage(
        env: JNIEnv,
        class: JClass,
        message: jstring,
    );
}