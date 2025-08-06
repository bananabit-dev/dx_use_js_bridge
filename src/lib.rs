use dioxus::core::use_drop;
use dioxus::prelude::*;
use dioxus::signals::{Readable, Writable, Signal};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

// Only import wasm-specific modules when targeting wasm
#[cfg(target_arch = "wasm32")]
use gloo_utils::format::JsValueSerdeExt;
#[cfg(target_arch = "wasm32")]
use js_sys;
#[cfg(target_arch = "wasm32")]
use serde_wasm_bindgen;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{prelude::Closure, JsValue};
#[cfg(target_arch = "wasm32")]
use web_sys;

// Import the android_bridge module
#[cfg(target_os = "android")]
pub mod android_bridge;

// Always import uuid when the feature is enabled
#[cfg(feature = "uuid")]
use uuid;

pub trait FromJs: for<'de> Deserialize<'de> + 'static {}
impl<T> FromJs for T where T: for<'de> Deserialize<'de> + 'static {}

#[derive(Clone)]
pub struct JsBridge<T: FromJs + Clone> {
    pub data: Signal<Option<T>>,
    pub error: Signal<Option<String>>,
    callback_id: Signal<String>,
}

impl<T: FromJs + Clone> JsBridge<T> {
    fn new(
        data: Signal<Option<T>>,
        error: Signal<Option<String>>,
        callback_id: Signal<String>,
    ) -> Self {
        Self {
            data,
            error,
            callback_id,
        }
    }

    pub fn get_data(&self) -> Option<T> {
        self.data.read().clone()
    }
    pub fn get_error(&self) -> Option<String> {
        self.error.read().clone()
    }
    pub fn callback_id(&self) -> String {
        self.callback_id.read().clone()
    }
    pub fn set_error(&mut self, error: Option<String>) {
        self.error.with_mut(|v| *v = error);
    }
    pub fn set_data(&mut self, data: Option<T>) {
        self.data.with_mut(|v| *v = data);
    }

    /// Rust → JS: Evaluate JS code (cross-platform via dioxus::html::document().eval)
    pub async fn eval(&mut self, js_code: &str) -> Result<(), String> {
        #[cfg(target_arch = "wasm32")]
        {
            dioxus::document::eval(js_code)
                .await
                .map(|_| ())
                .map_err(|e| format!("JS eval error: {:?}", e))
        }
        
        #[cfg(not(target_arch = "wasm32"))]
        {
            // For non-WASM targets, we need to handle this differently
            #[cfg(target_os = "android")]
            {
                // For Android, we'll use the JNI bridge to evaluate JS
                self.eval_android(js_code).await
            }
            
            #[cfg(not(target_os = "android"))]
            {
                // For Desktop, we can use dioxus::document::eval
                dioxus::document::eval(js_code)
                    .await
                    .map(|_| ())
                    .map_err(|e| format!("JS eval error: {:?}", e))
            }
        }
    }

    #[cfg(target_os = "android")]
    async fn eval_android(&mut self, js_code: &str) -> Result<(), String> {
        use crate::android_bridge;
        
        // Send the JavaScript code to be evaluated on the Android side
        android_bridge::eval_js(js_code).await
    }

    pub async fn send_to_js<S: Serialize>(&mut self, data: &S) -> Result<(), String> {
        let json_data =
            serde_json::to_string(data).map_err(|e| format!("Serialization error: {}", e))?;
        
        // Platform-specific implementations
        #[cfg(target_arch = "wasm32")]
        {
            let js_code = format!(
                "if (window.__dioxus_bridge_{}) {{ window.__dioxus_bridge_{}({}); }}",
                self.callback_id(),
                self.callback_id(),
                json_data
            );
            self.eval(&js_code).await
        }
        
        #[cfg(target_os = "android")]
        {
            // For Android, use the JNI bridge
            self.send_to_js_android(&json_data).await
        }
        
        #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
        {
            // For Desktop
            let js_code = format!(
                "if (window.__dioxus_bridge_{}) {{ window.__dioxus_bridge_{}({}); }}",
                self.callback_id(),
                self.callback_id(),
                json_data
            );
            self.eval(&js_code).await
        }
    }

    #[cfg(target_os = "android")]
    async fn send_to_js_android(&mut self, json_data: &str) -> Result<(), String> {
        use crate::android_bridge;
        
        // Create a message that includes the callback ID and data
        let message = format!(
            "{{\"callback_id\":\"{}\",\"data\":{}}}",
            self.callback_id(),
            json_data
        );
        
        // Send the message to Java/Kotlin via the JNI bridge
        android_bridge::send_to_java(message).await
    }
}

pub fn use_js_bridge<T>() -> JsBridge<T>
where
    T: FromJs + Clone + Debug + 'static,
{
    let data: Signal<Option<T>> = use_signal(|| None);
    let error: Signal<Option<String>> = use_signal(|| None);

    // Generate callback_id in a platform-specific way
    let callback_id = use_signal(|| {
        #[cfg(feature = "uuid")]
        {
            uuid::Uuid::new_v4().to_string().replace("-", "_")
        }
        #[cfg(all(target_arch = "wasm32", not(feature = "uuid")))]
        {
            // This code only compiles for WASM targets
            let random_part: String = js_sys::Math::random().to_string().chars().skip(2).collect();
            format!("callback_{}_{}", js_sys::Date::now(), random_part)
        }
        #[cfg(not(any(target_arch = "wasm32", feature = "uuid")))]
        {
            // For non-WASM targets without uuid feature
            use std::time::{SystemTime, UNIX_EPOCH};
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis();
            format!("callback_{}", timestamp)
        }
    });

    let bridge = JsBridge::new(data.clone(), error.clone(), callback_id.clone());

    // --- Web: Register JS callback ---
    #[cfg(target_arch = "wasm32")]
    {
        let mut bridge_for_effect = bridge.clone();
        use_effect(move || {
            let callback_id_str = bridge_for_effect.callback_id();
            let mut bridge_for_callback = bridge_for_effect.clone();
            let callback = Closure::<dyn FnMut(JsValue)>::new(move |val: JsValue| {
                // Try to deserialize directly using serde-wasm-bindgen
                match val.into_serde() {
                    Ok(parsed) => {
                        bridge_for_callback.set_data(Some(parsed));
                        bridge_for_callback.set_error(None);
                        return;
                    }
                    Err(_) => {
                        // If direct deserialization fails, try to convert to string first
                        let js_string = js_sys::JsString::from(val);
                        let rust_string = String::from(js_string);
                        match serde_json::from_str::<T>(&rust_string) {
                            Ok(parsed) => {
                                bridge_for_callback.set_data(Some(parsed));
                                bridge_for_callback.set_error(None);
                                return;
                            }
                            Err(e) => bridge_for_callback
                                .set_error(Some(format!("Deserialization error: {e}"))),
                        }
                    }
                }
            });
            let window = web_sys::window().expect("no global window");
            let callback_name = format!("__dioxus_bridge_{}", callback_id_str);
            js_sys::Reflect::set(&window, &callback_name.into(), callback.as_ref())
                .expect("failed to set callback");
            callback.forget();
        });
        let bridge_for_destroy = bridge.clone();
        use_drop(move || {
            if let Some(window) = web_sys::window() {
                let callback_name = format!("__dioxus_bridge_{}", bridge_for_destroy.callback_id());
                let _ = js_sys::Reflect::delete_property(&window, &callback_name.into());
            }
        });
    }

    // --- Desktop: Register JS callback (Wry) ---
    #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
    {
        let mut bridge_for_effect = bridge.clone();
        use_effect(move || {
            // For Dioxus Desktop, inject a JS callback in your HTML or via eval.
            let callback_id_str = bridge_for_effect.callback_id();
            let js_code = format!(
                "window.__dioxus_bridge_{} = function(data) {{
                    if (window.__dioxus_bridge_callback) {{
                        window.__dioxus_bridge_callback('{}', JSON.stringify(data));
                    }}
                }}",
                callback_id_str, callback_id_str
            );
            
            // Clone the bridge before moving it into the closure
            let mut bridge_clone = bridge_for_effect.clone();
            spawn(async move {
                if let Err(e) = bridge_clone.eval(&js_code).await {
                    eprintln!("Failed to inject desktop bridge function: {}", e);
                }
            });
        });
    }

    // --- Android: Register JNI callback with channel to main thread ---
    #[cfg(target_os = "android")]
    {
        use crate::android_bridge::{register_callback, unregister_callback};
        use std::sync::mpsc::channel;

        let (tx, rx) = channel::<String>();
        let callback_id_str = bridge.callback_id();

        register_callback(
            callback_id_str.clone(),
            move |json: String| {
                let _ = tx.send(json);
            },
        );

        let mut data = data.clone();
        let mut error = error.clone();
        use_effect(move || {
            while let Ok(json) = rx.try_recv() {
                match serde_json::from_str::<T>(&json) {
                    Ok(parsed) => {
                        data.with_mut(|v| *v = Some(parsed));
                        error.with_mut(|v| *v = None);
                    }
                    Err(e) => {
                        error.with_mut(|v| *v = Some(format!("Deserialization error: {e}")));
                    }
                }
            }
        });

        // Also inject a JS function for Android
        let mut bridge_for_effect = bridge.clone();
        use_effect(move || {
            let callback_id_str = bridge_for_effect.callback_id();
            let js_code = format!(
                "window.__dioxus_bridge_{} = function(data) {{
                    // Try JsBridge first
                    if (window.JsBridge && window.JsBridge.postMessage) {{
                        window.JsBridge.postMessage('{}', JSON.stringify(data));
                        return;
                    }}
                    
                    // Try RustBridge as fallback
                    if (window.RustBridge && window.RustBridge.postMessage) {{
                        window.RustBridge.postMessage('{}', JSON.stringify(data));
                        return;
                    }}
                    
                    // Try alternative method with custom event
                    try {{
                        var event = new CustomEvent('rustBridgeMessage', {{
                            detail: {{
                                callbackId: '{}',
                                data: data
                            }}
                        }});
                        window.dispatchEvent(event);
                    }} catch (e) {{
                        console.error('Failed to send message via custom event:', e);
                    }}
                    
                    // Log if no method works
                    console.warn('No bridge available for callback {}');
                }}",
                callback_id_str, callback_id_str, callback_id_str, callback_id_str, callback_id_str
            );
            
            // Clone the bridge before moving it into the closure and make it mutable
            let mut bridge_clone = bridge_for_effect.clone();
            spawn(async move {
                if let Err(e) = bridge_clone.eval(&js_code).await {
                    eprintln!("Failed to inject android bridge function: {}", e);
                }
            });
        });

        let callback_id = bridge.callback_id();
        use_drop(move || {
            unregister_callback(&callback_id);
        });
    }

    bridge
}