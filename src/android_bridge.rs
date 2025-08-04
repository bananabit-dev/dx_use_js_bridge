use jni::sys;
use jni::JavaVM;
use jni::objects::{JClass, JObject, JString, JValue};
use jni::JNIEnv;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{Mutex, Once};

// Global static to hold callback functions.
static CALLBACKS: Lazy<Mutex<HashMap<String, Box<dyn Fn(String) + Send + Sync>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

// Global static to hold the JavaVM pointer using atomic for better thread safety.
static GLOBAL_JAVA_VM: AtomicPtr<sys::JavaVM> = AtomicPtr::new(ptr::null_mut());

/// This function is called when the native library is loaded.
/// It stores the JavaVM pointer for later use.
#[no_mangle]
pub unsafe extern "C" fn JNI_OnLoad(
    vm: *mut sys::JavaVM,
    _reserved: *mut std::ffi::c_void,
) -> sys::jint {
    // Store the JavaVM pointer atomically
    GLOBAL_JAVA_VM.store(vm, Ordering::SeqCst);
    
    // Print debug info
    eprintln!("JNI_OnLoad called, stored JavaVM pointer: {:?}", vm);
    
    sys::JNI_VERSION_1_6
}

/// On Android, retrieve the JavaVM from our stored global variable.
#[cfg(target_os = "android")]
fn get_java_vm() -> Option<JavaVM> {
    unsafe {
        // First try to get it from our stored pointer
        let vm_ptr = GLOBAL_JAVA_VM.load(Ordering::SeqCst);
        eprintln!("Attempting to get JavaVM from stored pointer: {:?}", vm_ptr);
        
        if !vm_ptr.is_null() {
            match JavaVM::from_raw(vm_ptr) {
                Ok(vm) => {
                    eprintln!("Successfully created JavaVM from stored pointer");
                    return Some(vm);
                }
                Err(e) => {
                    eprintln!("Failed to create JavaVM from stored pointer: {:?}", e);
                }
            }
        } else {
            eprintln!("Stored JavaVM pointer is null");
        }
        
        None
    }
}

/// Registers a callback function under the provided identifier.
pub fn register_callback<F>(id: String, callback: F)
where
    F: Fn(String) + Send + Sync + 'static,
{
    let mut callbacks = CALLBACKS.lock().unwrap();
    callbacks.insert(id, Box::new(callback));
}

/// Unregisters the callback function associated with the provided identifier.
pub fn unregister_callback(id: &str) {
    let mut callbacks = CALLBACKS.lock().unwrap();
    callbacks.remove(id);
}

/// Evaluates JavaScript on Android by calling the static method `evalJs` on
/// the Kotlin class "io.github.memkit.RustBridge".
pub async fn eval_js(js_code: &str) -> Result<(), String> {
    eprintln!("Attempting to evaluate JS: {}", js_code);
    
    // Retrieve the JavaVM.
    let vm = get_java_vm().ok_or("Failed to get JavaVM")?;
    eprintln!("Successfully got JavaVM for eval_js");
    
    // Attach the current thread to the JVM.
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| format!("Failed to attach to JVM: {:?}", e))?;
    eprintln!("Successfully attached to JVM");
    
    // Find the class "io/github/memkit/RustBridge".
    let class_name = "io/github/memkit/RustBridge";
    let class = env
        .find_class(class_name)
        .map_err(|e| format!("Failed to find class {}: {:?}", class_name, e))?;
    eprintln!("Successfully found class: {}", class_name);
    
    // Create a Java string from js_code.
    let js_string = env
        .new_string(js_code)
        .map_err(|e| format!("Failed to create Java string: {:?}", e))?;
    eprintln!("Successfully created Java string");
    
    // Convert the JString into a JObject.
    let js_obj: JObject = JObject::from(js_string);
    
    // Prepare the argument list.
    let args = [JValue::Object(&js_obj)];
    
    // Call the static method "evalJs".
    env.call_static_method(class, "evalJs", "(Ljava/lang/String;)V", &args)
        .map_err(|e| format!("Failed to call evalJs: {:?}", e))?;
    eprintln!("Successfully called evalJs method");
    
    // Check for any exceptions thrown by the JVM.
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
    
    eprintln!("Successfully evaluated JS: {}", js_code);
    Ok(())
}

/// Sends data to Kotlin by calling the static method `onMessageFromRust` on
/// the Kotlin class "io.github.memkit.RustBridge".
pub async fn send_to_java(message: String) -> Result<(), String> {
    eprintln!("Attempting to send message to Kotlin: {}", message);
    
    let vm = get_java_vm().ok_or("Failed to get JavaVM")?;
    eprintln!("Successfully got JavaVM for send_to_java");
    
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| format!("Failed to attach to JVM: {:?}", e))?;
    eprintln!("Successfully attached to JVM");
    
    let class_name = "io/github/memkit/RustBridge";
    let class = env
        .find_class(class_name)
        .map_err(|e| format!("Failed to find class {}: {:?}", class_name, e))?;
    eprintln!("Successfully found class: {}", class_name);
    
    let msg_string = env
        .new_string(&message)
        .map_err(|e| format!("Failed to create Java string: {:?}", e))?;
    eprintln!("Successfully created Java string");
    
    let msg_obj: JObject = JObject::from(msg_string);
    let args = [JValue::Object(&msg_obj)];
    
    env.call_static_method(
        class,
        "onMessageFromRust",
        "(Ljava/lang/String;)V",
        &args,
    )
    .map_err(|e| format!("Failed to call onMessageFromRust: {:?}", e))?;
    eprintln!("Successfully called onMessageFromRust method");
    
    if env
        .exception_check()
        .map_err(|e| format!("Failed to check for exceptions: {:?}", e))?
    {
        env.exception_describe()
            .map_err(|e| format!("Failed to describe exception: {:?}", e))?;
        env.exception_clear()
            .map_err(|e| format!("Failed to clear exception: {:?}", e))?;
        return Err("Sending message to Kotlin threw an exception".to_string());
    }
    
    eprintln!("Successfully sent message to Kotlin: {}", message);
    Ok(())
}

/// This JNI function is called from Kotlin when a message is received.
/// It converts the incoming Java strings to Rust strings and then invokes the
/// registered callback for the provided callback ID.
#[no_mangle]
pub extern "system" fn Java_io_github_memkit_RustBridge_onMessageFromJava(
    mut env: JNIEnv,
    _class: JClass,
    callback_id: JString,
    json_data: JString,
) {
    eprintln!("Received message from Kotlin - callback_id length: {}, json_data length: {}", 
              env.get_string(&callback_id).map(|s| s.to_string_lossy().len()).unwrap_or(0),
              env.get_string(&json_data).map(|s| s.to_string_lossy().len()).unwrap_or(0));
    
    let callback_id_rust = match env.get_string(&callback_id) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("Failed to get callback_id string");
            return;
        }
    };
    let callback_id_str = match callback_id_rust.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => {
            eprintln!("Failed to convert callback_id to str");
            return;
        }
    };
    
    let json_data_rust = match env.get_string(&json_data) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("Failed to get json_data string");
            return;
        }
    };
    let json_data_str = match json_data_rust.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => {
            eprintln!("Failed to convert json_data to str");
            return;
        }
    };
    
    eprintln!("Processing message - callback_id: {}, json_data length: {}", callback_id_str, json_data_str.len());
    
    let callbacks = CALLBACKS.lock().unwrap();
    if let Some(callback) = callbacks.get(&callback_id_str) {
        callback(json_data_str);
        eprintln!("Successfully called callback for: {}", callback_id_str);
    } else {
        eprintln!("No callback found for: {}", callback_id_str);
    }
}

/// JNI function to register the main activity instance
#[no_mangle]
pub extern "system" fn Java_io_github_memkit_RustBridge_registerInstance(
    _env: JNIEnv,
    _class: JClass,
    _activity: JObject,
) {
    eprintln!("registerInstance called - activity registered");
    // This function is called when MainActivity registers itself
    // We don't need to do anything here as the JavaVM is already stored in JNI_OnLoad
}
