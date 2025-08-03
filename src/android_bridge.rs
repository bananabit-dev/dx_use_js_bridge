use jni::{
    objects::{JClass, JObject, JString, JValue},
    sys::jstring,
    JNIEnv, JavaVM,
};
use std::collections::HashMap;
use std::ffi::CString;
use std::os::raw::c_char;
use std::ptr;
use std::sync::Mutex;
use std::thread;

// Global storage for callbacks
static CALLBACKS: Mutex<HashMap<String, Box<dyn Fn(String) + Send + Sync>>> =
    Mutex::new(HashMap::new());

// Get the JavaVM instance
fn get_java_vm() -> Option<JavaVM> {
    // This should be initialized when the JNI library is loaded
    // In a real implementation, you would store this when the library is loaded
    unsafe {
        // This is a placeholder - in a real implementation, you would get the VM from JNI_OnLoad
        // For now, we'll try to get it from the current thread
        jni::JavaVM::get_java_vm_pointer()
            .map(|ptr| unsafe { JavaVM::from_raw(ptr) })
            .ok()
    }
}

// Register a callback for a given ID
pub fn register_callback<F>(id: String, callback: F)
where
    F: Fn(String) + Send + Sync + 'static,
{
    let mut callbacks = CALLBACKS.lock().unwrap();
    callbacks.insert(id, Box::new(callback));
}

// Unregister a callback
pub fn unregister_callback(id: &str) {
    let mut callbacks = CALLBACKS.lock().unwrap();
    callbacks.remove(id);
}

// Evaluate JavaScript on Android
pub async fn eval_js(js_code: &str) -> Result<(), String> {
    // Get the JavaVM
    let vm = get_java_vm().ok_or("Failed to get JavaVM")?;
    
    // Attach to the current thread
    let env = vm.attach_current_thread()
        .map_err(|e| format!("Failed to attach to JVM: {:?}", e))?;
    
    // Find the RustBridge class
    let class_name = "io/github/memkit/RustBridge";
    let class = env.find_class(class_name)
        .map_err(|e| format!("Failed to find class {}: {:?}", class_name, e))?;
    
    // Get the method ID for evalJs
    let method_id = env.get_static_method_id(
        class,
        "evalJs",
        "(Ljava/lang/String;)V"
    ).map_err(|e| format!("Failed to get method ID: {:?}", e))?;
    
    // Create a Java string from the JavaScript code
    let js_code_jstring = env.new_string(js_code)
        .map_err(|e| format!("Failed to create Java string: {:?}", e))?;
    
    // Call the static method
    env.call_static_void_method(class, method_id, &[JValue::Object(&js_code_jstring.into())])
        .map_err(|e| format!("Failed to call evalJs: {:?}", e))?;
    
    // Check for exceptions
    if env.exception_check().map_err(|e| format!("Failed to check for exceptions: {:?}", e))? {
        env.exception_describe()
            .map_err(|e| format!("Failed to describe exception: {:?}", e))?;
        env.exception_clear()
            .map_err(|e| format!("Failed to clear exception: {:?}", e))?;
        return Err("JavaScript evaluation threw an exception".to_string());
    }
    
    Ok(())
}

// Send data to Java/Kotlin
pub async fn send_to_java(message: String) -> Result<(), String> {
    // Get the JavaVM
    let vm = get_java_vm().ok_or("Failed to get JavaVM")?;
    
    // Attach to the current thread
    let env = vm.attach_current_thread()
        .map_err(|e| format!("Failed to attach to JVM: {:?}", e))?;
    
    // Find the RustBridge class
    let class_name = "io/github/memkit/RustBridge";
    let class = env.find_class(class_name)
        .map_err(|e| format!("Failed to find class {}: {:?}", class_name, e))?;
    
    // Get the method ID for onMessageFromRust
    let method_id = env.get_static_method_id(
        class,
        "onMessageFromRust",
        "(Ljava/lang/String;)V"
    ).map_err(|e| format!("Failed to get method ID: {:?}", e))?;
    
    // Create a Java string from the message
    let message_jstring = env.new_string(&message)
        .map_err(|e| format!("Failed to create Java string: {:?}", e))?;
    
    // Call the static method
    env.call_static_void_method(class, method_id, &[JValue::Object(&message_jstring.into())])
        .map_err(|e| format!("Failed to call onMessageFromRust: {:?}", e))?;
    
    // Check for exceptions
    if env.exception_check().map_err(|e| format!("Failed to check for exceptions: {:?}", e))? {
        env.exception_describe()
            .map_err(|e| format!("Failed to describe exception: {:?}", e))?;
        env.exception_clear()
            .map_err(|e| format!("Failed to clear exception: {:?}", e))?;
        return Err("Sending message to Java threw an exception".to_string());
    }
    
    Ok(())
}

// JNI function to be called from Java/Kotlin
#[no_mangle]
pub extern "system" fn Java_io_github_memkit_RustBridge_onMessageFromJava(
    env: JNIEnv,
    _class: JClass,
    callback_id: JString,
    json_data: JString,
) {
    // Convert Java strings to Rust strings
    let callback_id_rust = match env.get_string(callback_id) {
        Ok(s) => s,
        Err(_) => return,
    };
    let callback_id_str = match callback_id_rust.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return,
    };
    
    let json_data_rust = match env.get_string(json_data) {
        Ok(s) => s,
        Err(_) => return,
    };
    let json_data_str = match json_data_rust.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return,
    };
    
    // Get the callback and call it
    let callbacks = CALLBACKS.lock().unwrap();
    if let Some(callback) = callbacks.get(&callback_id_str) {
        callback(json_data_str);
    }
}