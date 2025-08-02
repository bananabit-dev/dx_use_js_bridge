use dioxus::core::use_drop;
use dioxus::prelude::*;
use dioxus_signals::{Readable, Writable};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

pub trait FromJs: for<'de> Deserialize<'de> + 'static {}
impl<T> FromJs for T where T: for<'de> Deserialize<'de> + 'static {}

#[derive(Clone)]
pub struct JsBridge<T: FromJs + Clone> {
    data: Signal<Option<T>>,
    error: Signal<Option<String>>,
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
        dioxus::document::eval(js_code)
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
                let callback_name = format!("__dioxus_bridge_{}", bridge_for_destroy.callback_id());
                let _ = web_sys::js_sys::Reflect::delete_property(&window, &callback_name.into());
            }
        });
    }

    // For desktop/mobile, you may need to set up the JS->Rust callback using Tauri commands or a custom JS interface.

    bridge
}
