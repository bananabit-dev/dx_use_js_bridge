use dioxus::core::use_drop;
use dioxus::prelude::*;
use dioxus_signals::{Readable, Writable};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

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
        dioxus::html::document()
            .eval(js_code)
            .await
            .map(|_| ())
            .map_err(|e| format!("JS eval error: {:?}", e))
    }

    pub async fn send_to_js<S: Serialize>(&mut self, data: &S) -> Result<(), String> {
        let json_data =
            serde_json::to_string(data).map_err(|e| format!("Serialization error: {}", e))?;
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
mod android_bridge {
    use super::*;
    use once_cell::sync::Lazy;
    use std::collections::HashMap;
    use std::sync::Mutex;

    static CALLBACKS: Lazy<Mutex<HashMap<String, Box<dyn Fn(String) + Send + Sync>>>> =
        Lazy::new(|| Mutex::new(HashMap::new()));

    pub fn register_callback<F: Fn(String) + Send + Sync + 'static>(id: String, cb: F) {
        CALLBACKS.lock().unwrap().insert(id, Box::new(cb));
    }
    pub fn unregister_callback(id: &str) {
        CALLBACKS.lock().unwrap().remove(id);
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
}

pub fn use_js_bridge<T>() -> JsBridge<T>
where
    T: FromJs + Clone + Debug + 'static,
{
    #[cfg(all(not(feature = "uuid"), target_arch = "wasm32"))]
    use js_sys;

    let data: Signal<Option<T>> = use_signal(|| None);
    let error: Signal<Option<String>> = use_signal(|| None);

    let callback_id = use_signal(move || {
        #[cfg(feature = "uuid")]
        {
            uuid::Uuid::new_v4().to_string().replace("-", "_")
        }
        #[cfg(all(not(feature = "uuid"), target_arch = "wasm32"))]
        {
            let random_part: String = js_sys::Math::random().to_string().chars().skip(2).collect();
            format!("callback_{}_{}", js_sys::Date::now(), random_part)
        }
        #[cfg(all(not(feature = "uuid"), not(target_arch = "wasm32")))]
        {
            format!("callback_{}", chrono::Utc::now().timestamp_millis())
        }
    });

    let bridge = JsBridge::new(data.clone(), error.clone(), callback_id.clone());

    // --- Web/Desktop: Register JS callback ---
    #[cfg(any(target_arch = "wasm32", feature = "desktop"))]
    {
        let mut bridge_for_effect = bridge.clone();
        use_effect(move || {
            #[cfg(target_arch = "wasm32")]
            {
                use wasm_bindgen::{prelude::Closure, JsValue};
                use web_sys::js_sys;
                let callback_id_str = bridge_for_effect.callback_id();
                let mut bridge_for_callback = bridge_for_effect.clone();
                let callback = Closure::<dyn FnMut(JsValue)>::new(move |val: JsValue| {
                    match serde_wasm_bindgen::from_value::<T>(val.clone()) {
                        Ok(parsed) => {
                            bridge_for_callback.set_data(Some(parsed));
                            bridge_for_callback.set_error(None);
                            return;
                        }
                        Err(_) => {}
                    }
                    if let Some(s) = val.as_string() {
                        match serde_json::from_str::<T>(&s) {
                            Ok(parsed) => {
                                bridge_for_callback.set_data(Some(parsed));
                                bridge_for_callback.set_error(None);
                                return;
                            }
                            Err(e) => bridge_for_callback
                                .set_error(Some(format!("Deserialization error: {e}"))),
                        }
                    } else {
                        bridge_for_callback.set_error(Some(
                            "Unsupported value type sent over JsBridge".to_string(),
                        ));
                    }
                });
                let window = web_sys::window().expect("no global window");
                let callback_name = format!("__dioxus_bridge_{}", callback_id_str);
                js_sys::Reflect::set(&window, &callback_name.into(), callback.as_ref())
                    .expect("failed to set callback");
                callback.forget();
            }
            #[cfg(feature = "desktop")]
            {
                // For Dioxus Desktop, inject a JS callback in your HTML or via eval.
                // Example: in your index.html, add:
                // <script>
                // window.__dioxus_bridge_callback = function(callbackId, payload) {
                //   if (window["__dioxus_bridge_" + callbackId]) {
                //     window["__dioxus_bridge_" + callbackId](payload);
                //   }
                // };
                // </script>
            }
        });
        #[cfg(target_arch = "wasm32")]
        {
            let bridge_for_destroy = bridge.clone();
            use_drop(move || {
                if let Some(window) = web_sys::window() {
                    let callback_name =
                        format!("__dioxus_bridge_{}", bridge_for_destroy.callback_id());
                    let _ = web_sys::js_sys::Reflect::delete_property(&window, &callback_name.into());
                }
            });
        }
    }

    // --- Android: Register JNI callback with channel to main thread ---
    #[cfg(target_os = "android")]
    {
        use self::android_bridge::{register_callback, unregister_callback};
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

        let callback_id = bridge.callback_id();
        use_drop(move || {
            unregister_callback(&callback_id);
        });
    }

    bridge
}