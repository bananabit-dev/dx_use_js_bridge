use dioxus::core::use_drop;
use dioxus::prelude::*;
use dioxus_signals::Readable;
use dioxus_signals::Writable;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[cfg(all(not(target_arch = "wasm32"), feature = "tauri"))]
use dioxus_desktop::use_window;
#[cfg(all(not(target_arch = "wasm32"), feature = "tauri"))]
use tauri::Window;

#[cfg(all(not(target_arch = "wasm32"), feature = "tauri"))]
use tauri::Manager;

// Trait for types that can be safely deserialized from JS
pub trait FromJs: for<'de> Deserialize<'de> + 'static {}
impl<T> FromJs for T where T: for<'de> Deserialize<'de> + 'static {}

#[derive(Clone)]
pub struct JsBridge<T: FromJs + Clone> {
    data: Signal<Option<T>>,
    error: Signal<Option<String>>,
    callback_id: Signal<String>,
    #[cfg(all(not(target_arch = "wasm32"), feature = "tauri"))]
    tauri_window: Window,
}

impl<T: FromJs + Clone> JsBridge<T> {
    fn new(
        data: Signal<Option<T>>,
        error: Signal<Option<String>>,
        callback_id: Signal<String>,
        #[cfg(all(not(target_arch = "wasm32"), feature = "tauri"))]
        tauri_window: Window,
    ) -> Self {
        Self {
            data,
            error,
            callback_id,
            #[cfg(all(not(target_arch = "wasm32"), feature = "tauri"))]
            tauri_window,
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
        self.error.with_mut(|v| {
            *v = error;
        });
    }

    pub fn set_data(&mut self, data: Option<T>) {
        self.data.with_mut(|v| {
            *v = data;
        });
    }

    /// Evaluates JavaScript code.
    pub async fn eval(&mut self, js_code: &str) -> Result<(), String> {
        // --- Web (wasm32) ---
        #[cfg(target_arch = "wasm32")]
        {
            web_sys::js_sys::eval(js_code)
                .map_err(|e| format!("JS eval error: {:?}", e))?;
            Ok(())
        }
        // --- Tauri (desktop) ---
        #[cfg(all(not(target_arch = "wasm32"), feature = "tauri"))]
        {
            self.tauri_window
                .eval(js_code)
                .map_err(|e| format!("Tauri JS eval error: {:?}", e))
        }
        // --- Android (native) ---
        #[cfg(all(not(target_arch = "wasm32"), target_os = "android"))]
        {
            println!("Android JS eval: {}", js_code);
            Ok(())
        }
        // --- Native fallback (no-op) ---
        #[cfg(all(
            not(target_arch = "wasm32"),
            not(feature = "tauri"),
            not(target_os = "android")
        ))]
        {
            Err(format!(
                "JS evaluation not supported on this platform. Code: {}",
                js_code
            ))
        }
    }

    /// Sends a serializable value to the JavaScript side.
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

// ... android_bridge module as before ...

/// A custom Dioxus hook for two-way communication with JavaScript.
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

    // --- Tauri: get window from Dioxus context ---
    #[cfg(all(not(target_arch = "wasm32"), feature = "tauri"))]
    let tauri_window = use_window();

    let bridge = JsBridge::new(
        data,
        error,
        callback_id,
        #[cfg(all(not(target_arch = "wasm32"), feature = "tauri"))]
        tauri_window,
    );

    // --- Web (wasm32): Set up JS callback ---
    #[cfg(target_arch = "wasm32")]
    {
        let mut bridge_for_effect = bridge.clone();
        use_effect(move || {
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
        });

        let bridge_for_destroy = bridge.clone();
        use_drop(move || {
            if let Some(window) = web_sys::window() {
                let callback_name =
                    format!("__dioxus_bridge_{}", bridge_for_destroy.callback_id());
                let _ = web_sys::js_sys::Reflect::delete_property(&window, &callback_name.into());
            }
        });
    }

    // --- Tauri (desktop): Set up JS->Rust callback via Tauri events ---
    #[cfg(all(not(target_arch = "wasm32"), feature = "tauri"))]
    {
        use std::sync::Arc;
        use std::sync::Mutex;

        let mut bridge_for_callback = bridge.clone();
        let callback_id = bridge.callback_id();

        // Listen for a Tauri event with the callback_id as the event name
        let window = bridge.tauri_window.clone();
        let data_signal = Arc::new(Mutex::new(bridge_for_callback.clone()));
        window.listen(callback_id.clone(), move |event| {
            let mut bridge = data_signal.lock().unwrap();
            match serde_json::from_value::<T>(event.payload().unwrap_or("").into()) {
                Ok(parsed) => {
                    bridge.set_data(Some(parsed));
                    bridge.set_error(None);
                }
                Err(e) => {
                    bridge.set_error(Some(format!("Deserialization error: {e}")));
                }
            }
        });
        // No drop needed: Tauri events are automatically cleaned up with the window.
    }

    // --- Android: Set up JS->Rust callback via WebView bridge ---
    #[cfg(all(not(target_arch = "wasm32"), target_os = "android"))]
    {
        use crate::android_bridge::{register_callback, unregister_callback};
        let mut bridge_for_callback = bridge.clone();
        let callback_id = bridge.callback_id();
        register_callback(callback_id.clone(), move |json: String| {
            match serde_json::from_str::<T>(&json) {
                Ok(parsed) => {
                    bridge_for_callback.set_data(Some(parsed));
                    bridge_for_callback.set_error(None);
                }
                Err(e) => {
                    bridge_for_callback.set_error(Some(format!("Deserialization error: {e}")));
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