use dioxus::core::use_drop;
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

// Trait for types that can be safely deserialized from JS
pub trait FromJs: for<'de> Deserialize<'de> + 'static {}
impl<T> FromJs for T where T: for<'de> Deserialize<'de> + 'static {}

// The JsBridge struct is a handle to the bridge's state and functions.
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

    // Evaluates JavaScript code. This is a core capability of the bridge.
    pub async fn eval(&mut self, js_code: &str) -> Result<(), String> {
       // #[cfg(feature = "web")]
        {
            web_sys::js_sys::eval(js_code).map_err(|e| format!("JS eval error: {:?}", e))?;
            Ok(())
        }

        #[cfg(not(feature = "web"))]
        {
            Err(format!(
                "JS evaluation not supported on this platform. Code: {}",
                js_code
            ))
        }
    }

    // Sends a serializable value to the JavaScript side.
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

/// A custom Dioxus hook for two-way communication with JavaScript.
/// This hook's responsibility is to set up the JS-to-Rust callback and provide a handle.
/// Any initialization logic should be handled by the component using the hook in a `use_effect`.
pub fn use_js_bridge<T>() -> JsBridge<T>
where
    T: FromJs + Clone + Debug + 'static,
{
    #[cfg(all(not(feature = "uuid"), feature = "web"))]
    use js_sys;

    let mut data: Signal<Option<T>> = use_signal(|| None);
    let mut error: Signal<Option<String>> = use_signal(|| None);

    let callback_id = use_signal(move || {
        #[cfg(feature = "uuid")]
        {
            uuid::Uuid::new_v4().to_string().replace("-", "_")
        }

        #[cfg(all(not(feature = "uuid"), feature = "web"))]
        {
            let random_part: String = js_sys::Math::random().to_string().chars().skip(2).collect();
            format!("callback_{}_{}", js_sys::Date::now(), random_part)
        }
        #[cfg(all(not(feature = "uuid"), not(feature = "web")))]
        {
            format!("callback_{}", chrono::Utc::now().timestamp_millis())
        }
    });

    let bridge = JsBridge::new(data, error, callback_id);

    // This effect runs once to set up the JS callback.
    let bridge_for_effect = bridge.clone();
    use_effect(move || {
        //#[cfg(feature = "web")]
        {
            use wasm_bindgen::{JsValue, prelude::Closure};
            use web_sys::js_sys;
            let callback_id_str = bridge_for_effect.callback_id();

            // Create and register the callback that JS will call.
            // inside the effect that installs the JS callback
            let callback = Closure::<dyn FnMut(JsValue)>::new(move |val: JsValue| {
                // Fast path: try to deserialize the value directly
                match serde_wasm_bindgen::from_value::<T>(val.clone()) {
                    Ok(parsed) => {
                        *data.write() = Some(parsed);
                        *error.write() = None;
                        return;
                    }
                    Err(_) => {} // fall through – might be a JSON string
                }

                // If we got here the value is NOT the right type.
                // Try again: maybe it's a JSON string.
                if let Some(s) = val.as_string() {
                    match serde_json::from_str::<T>(&s) {
                        Ok(parsed) => {
                            *data.write() = Some(parsed);
                            *error.write() = None;
                            return;
                        }
                        Err(e) => *error.write() = Some(format!("Deserialization error: {e}")),
                    }
                } else {
                    *error.write() = Some("Unsupported value type sent over JsBridge".to_string());
                }
            });

            let window = web_sys::window().expect("no global window");
            let callback_name = format!("__dioxus_bridge_{}", callback_id_str);
            js_sys::Reflect::set(&window, &callback_name.into(), callback.as_ref())
                .expect("failed to set callback");
            callback.forget();
        }
    });

    // Use `use_drop` for cleanup logic, as this is the modern Dioxus API.
    let bridge_for_destroy = bridge.clone();
    use_drop(move || {
        //#[cfg(feature = "web")]
        {
            if let Some(window) = web_sys::window() {
                let callback_name = format!("__dioxus_bridge_{}", bridge_for_destroy.callback_id());
                let _ = web_sys::js_sys::Reflect::delete_property(&window, &callback_name.into());
            }
        }
    });

    bridge
}
