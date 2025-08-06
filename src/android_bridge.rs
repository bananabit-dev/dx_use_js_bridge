use jni::objects::{JClass, JObject, JString, JValue};
use jni::sys;
use jni::{JNIEnv, JavaVM};
use once_cell::sync::{Lazy, OnceCell};
use std::collections::HashMap;
use std::ptr;
use std::sync::Once;

use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Mutex;

// Pending queue
static PENDING_JS: Lazy<Mutex<Vec<String>>> = Lazy::new(|| Mutex::new(Vec::new()));

pub fn queue_js(json: String) {
    eprintln!("ANDROID: queue_js (no JVM yet), len={}…", json.len().min(80));
    PENDING_JS.lock().unwrap().push(json);
}

// A one-time guard and background flusher that periodically checks for JVM and flushes the queue.
static STARTED_FALLBACK_FLUSH: Once = Once::new();

pub fn start_fallback_flusher() {
    STARTED_FALLBACK_FLUSH.call_once(|| {
        std::thread::spawn(|| {
            // Try ~5 seconds, every 100ms
            for _ in 0..150 {
                if get_java_vm().is_some() {
                    let _ = std::panic::catch_unwind(|| {
                        futures::executor::block_on(async {
                            crate::android_bridge::try_flush_pending_js().await;
                        });
                    });
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        });
    });
}

// Flush helper that does NOT assume tokio exists.
// It iterates the queue and uses a small local async executor when available.
pub async fn try_flush_pending_js() {
    if get_java_vm().is_none() {
        eprintln!("ANDROID: try_flush_pending_js -> JVM not ready");
        return;
    }
    let mut pending = PENDING_JS.lock().unwrap();
    if pending.is_empty() {
        eprintln!("ANDROID: try_flush_pending_js -> nothing to flush");
        return;
    }
    let items: Vec<String> = pending.drain(..).collect();
    drop(pending);
    eprintln!(
        "ANDROID: try_flush_pending_js -> flushing {} command(s)",
        items.len()
    );

    for json in items {
        if let Err(e) = send_json_to_js_with_queue(json).await {
            eprintln!("ANDROID: flush send error: {}", e);
        }
    }
}

// Callbacks and JavaVM storage unchanged...
static CALLBACKS: Lazy<Mutex<HashMap<String, Box<dyn Fn(String) + Send + Sync>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

// Keep these at module scope in android_bridge.rs
pub fn register_callback<F>(id: String, callback: F)
where
    F: Fn(String) + Send + Sync + 'static,
{
    let mut callbacks = CALLBACKS.lock().unwrap();
    callbacks.insert(id, Box::new(callback));
}

pub fn unregister_callback(id: &str) {
    let mut callbacks = CALLBACKS.lock().unwrap();
    callbacks.remove(id);
}

static GLOBAL_JAVA_VM_CELL: OnceCell<JavaVM> = OnceCell::new();

pub unsafe fn store_java_vm(vm: *mut sys::JavaVM) {
    // Convert raw pointer to JavaVM (safe via from_raw) and store into OnceCell
    if let Ok(vm_obj) = JavaVM::from_raw(vm) {
        let _ = GLOBAL_JAVA_VM_CELL.set(vm_obj);
        eprintln!("Stored JavaVM in OnceCell from raw pointer: {:?}", vm);
    } else {
        eprintln!("Failed to create JavaVM from raw pointer");
    }
}

#[no_mangle]
pub unsafe extern "C" fn JNI_OnLoad(vm: *mut sys::JavaVM, _reserved: *mut std::ffi::c_void) -> sys::jint {
    store_java_vm(vm);
    eprintln!("JNI_OnLoad called, stored JavaVM pointer: {:?}", vm);

    // Spawn a lightweight thread; prefer an async executor if available.
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(50));

        // If tokio is available on Android, use it; otherwise use a minimal executor.
        #[cfg(all(target_os = "android", feature = "tokio-runtime"))]
        {
            let rt = tokio::runtime::Runtime::new().expect("tokio rt");
            rt.block_on(async { crate::android_bridge::try_flush_pending_js().await });
        }
        #[cfg(not(all(target_os = "android", feature = "tokio-runtime")))]
        {
            futures::executor::block_on(async { crate::android_bridge::try_flush_pending_js().await });
        }
    });

    sys::JNI_VERSION_1_6
}

#[no_mangle]
pub unsafe extern "C" fn Java_dev_dioxus_main_JsBridge_registerInstance(
    env: JNIEnv,
    _class: JClass,
    activity: JObject,
) {
    match env.get_java_vm() {
        Ok(vm) => {
            eprintln!("JsBridge_registerInstance: confirmed JVM access");
            // Store VM in OnceCell so get_java_vm() becomes available immediately
            let _ = GLOBAL_JAVA_VM_CELL.set(vm);
        }
        Err(e) => eprintln!("JsBridge_registerInstance: get_java_vm failed: {:?}", e),
    }
    eprintln!("JsBridge_registerInstance activity: {:?}", activity);

    // Ensure fallback flusher starts in case JNI_OnLoad/registerInstance timing differs
    start_fallback_flusher();

    // Flush again after Activity/WebView init
    std::thread::spawn(|| {
        #[cfg(all(target_os = "android", feature = "tokio-runtime"))]
        {
            let rt = tokio::runtime::Runtime::new().expect("tokio rt");
            rt.block_on(async { crate::android_bridge::try_flush_pending_js().await });
        }
        #[cfg(not(all(target_os = "android", feature = "tokio-runtime")))]
        {
            futures::executor::block_on(async { crate::android_bridge::try_flush_pending_js().await });
        }
    });

    // Force an immediate flush once Activity is registered
    #[cfg(not(all(target_os = "android", feature = "tokio-runtime")))]
    {
        futures::executor::block_on(async {
            crate::android_bridge::try_flush_pending_js().await;
        });
    }
    #[cfg(all(target_os = "android", feature = "tokio-runtime"))]
    {
        let rt = tokio::runtime::Runtime::new().expect("tokio rt");
        rt.block_on(async {
            crate::android_bridge::try_flush_pending_js().await;
        });
    }
}

#[cfg(target_os = "android")]
pub fn get_java_vm() -> Option<&'static JavaVM> {
    GLOBAL_JAVA_VM_CELL.get()
}

// ---------------- JNI helpers: eval_js / send_to_java ----------------

#[cfg(target_os = "android")]
pub async fn eval_js(js_code: &str) -> Result<(), String> {
    eprintln!("Attempting to evaluate JS: {}", js_code);

    let vm = get_java_vm().ok_or("Failed to get JavaVM")?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| format!("Failed to attach to JVM: {:?}", e))?;

    let class_name = "dev/dioxus/main/MainActivity";
    let class = env
        .find_class(class_name)
        .map_err(|e| format!("Failed to find class {}: {:?}", class_name, e))?;

    let js_string = env
        .new_string(js_code)
        .map_err(|e| format!("Failed to create Java string: {:?}", e))?;

    let js_obj: JObject = JObject::from(js_string);
    let args = [JValue::Object(&js_obj)];

    env.call_static_method(class, "evalJs", "(Ljava/lang/String;)V", &args)
        .map_err(|e| format!("Failed to call evalJs: {:?}", e))?;

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

#[cfg(target_os = "android")]
pub async fn send_to_java(message: String) -> Result<(), String> {
    eprintln!("Attempting to send message to Kotlin: {}", message);

    let vm = get_java_vm().ok_or("Failed to get JavaVM")?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| format!("Failed to attach to JVM: {:?}", e))?;

    let class_name = "dev/dioxus/main/MainActivity";
    let class = env
        .find_class(class_name)
        .map_err(|e| format!("Failed to find class {}: {:?}", class_name, e))?;

    let msg_string = env
        .new_string(&message)
        .map_err(|e| format!("Failed to create Java string: {:?}", e))?;

    let msg_obj: JObject = JObject::from(msg_string);
    let args = [JValue::Object(&msg_obj)];

    env.call_static_method(class, "onMessageFromRust", "(Ljava/lang/String;)V", &args)
        .map_err(|e| format!("Failed to call onMessageFromRust: {:?}", e))?;

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

    Ok(())
}

// ---------------- JNI callback entrypoint from Kotlin ----------------

#[no_mangle]
pub extern "system" fn Java_dev_dioxus_main_JsBridge_onMessageFromJava(
    mut env: JNIEnv,
    _class: JClass,
    callback_id: JString,
    json_data: JString,
) {
    eprintln!(
        "Received message from Kotlin - callback_id length: {}, json_data length: {}",
        env.get_string(&callback_id)
            .map(|s| s.to_string_lossy().len())
            .unwrap_or(0),
        env.get_string(&json_data)
            .map(|s| s.to_string_lossy().len())
            .unwrap_or(0)
    );

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

    let callbacks = CALLBACKS.lock().unwrap();
    if let Some(callback) = callbacks.get(&callback_id_str) {
        callback(json_data_str);
        eprintln!("Successfully called callback for: {}", callback_id_str);
    } else {
        eprintln!("No callback found for: {}", callback_id_str);
    }
}

// Public helper used internally to send JSON commands to JS with safe queue wrapper.
// This stays in dx_use_js_bridge so we don't depend on external crate modules.
#[cfg(target_os = "android")]
pub async fn send_json_to_js_with_queue(json: String) -> Result<(), String> {
    let js_code = format!(
        r#"(function () {{
            const c = {json};
            if (typeof window.dispatchStageCommand !== 'function') {{
                window._stageCmdQueue = window._stageCmdQueue || [];
                window._stageCmdQueue.push(c);
                console.log('[AndroidBridge] queued command:', c?.type);
            }} else {{
                console.log('[AndroidBridge] dispatching command:', c?.type);
                window.dispatchStageCommand(c);
            }}
        }})();"#
    );
    eval_js(&js_code).await
}
