Dioxus JS Bridge (dx_use_js_bridge)

A simple, platform-agnostic hook for two-way communication between Dioxus applications and JavaScript. This library allows you to send data from your Rust components to JavaScript and receive data back, using any serializable data type.
Features

    Seamless Two-Way Communication: Effortlessly send data from Rust to JS and receive data from JS in your Dioxus components.

    Hook-based API: Integrates smoothly into your components with a simple use_js_bridge hook.

    Type-Safe Generics: Define the exact data structure you expect from JavaScript, and the bridge will handle the deserialization. Works with any type that implements serde::Serialize and serde::Deserialize.

    Platform-Agnostic: Works on the web (wasm32) out-of-the-box. For other platforms (like desktop), the bridge provides non-functional stubs, preventing compilation errors.

    Collision-Free: Automatically generates unique IDs for each bridge instance to prevent multiple bridges from interfering with each other.

    Optional UUIDs: For more robust unique IDs, you can enable the uuid feature.

Installation

Add the library to your Cargo.toml file.

[dependencies]
dx_use_js_bridge = "0.1.0" # Replace with the latest version

The library has one optional feature, uuid, which can be enabled for stronger unique ID generation for each bridge instance.

# In your main application's Cargo.toml
[dependencies]
dx_use_js_bridge = { version = "0.1.0", features = ["uuid"] }

How to Use
1. In Your Rust Component

Use the use_js_bridge hook inside your Dioxus component. Specify the data type you expect to receive from JavaScript as the generic argument.

use dx_use_js_bridge::use_js_bridge;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct UserData {
    name: String,
    age: u32,
}

#[component]
fn MyComponent() -> Element {
    // Create a bridge to receive UserData from JS
    let user_bridge = use_js_bridge::<UserData>();

    // Create another bridge to receive simple strings
    let message_bridge = use_js_bridge::<String>();

    // ... rest of your component
}

2. Calling from JavaScript

To send data from JavaScript to Rust, call the global window.__dioxus_bridge_{callback_id} function. You can get the callback_id from the bridge instance in your Rust code.

// Get the callback ID from your Rust code
const callbackId = "your_bridge_callback_id"; // e.g., from a log or another JS call

// Send a string
window[`__dioxus_bridge_${callbackId}`]("Hello from JavaScript!");

// Send a complex object
window[`__dioxus_bridge_${callbackId}`]({
    name: "John Doe",
    age: 30
});

3. Running an Initialization Script

It's common to set up listeners or timers in JavaScript. You can do this by running an initial script in a use_effect hook.

// In your component
let message_bridge = use_js_bridge::<String>();
let message_bridge_for_effect = message_bridge.clone();

use_effect(move || {
    let mut bridge = message_bridge_for_effect.clone();
    spawn(async move {
        let js_code = format!(
            r#"
            console.log("Initializing message bridge with ID: {0}");
            setTimeout(() => {{
                if (window.__dioxus_bridge_{0}) {{
                    window.__dioxus_bridge_{0}("Hello from JavaScript!");
                }}
            }}, 2000);
            "#,
            bridge.callback_id()
        );
        if let Err(e) = bridge.eval(&js_code).await {
            bridge.error.set(Some(e));
        }
    });
});

4. Sending Data from Rust to JavaScript

You can also send data from Rust to your JavaScript code. This is useful for triggering actions or updating state on the JS side.

// In an onclick handler or other event
let mut bridge = user_bridge.clone();
spawn(async move {
    let data_to_send = UserData {
        name: "Alice".to_string(),
        age: 25,
    };
    // This requires a corresponding JS function to be listening
    if let Err(e) = bridge.send_to_js(&data_to_send).await {
        bridge.error.set(Some(e));
    }
});

5. Reading Data and Errors

Use the .get_data() and .get_error() methods to reactively read the state of the bridge in your rsx! macro.

rsx! {
    // Display received user data
    if let Some(user) = user_bridge.get_data() {
        div {
            "Received User: {user.name}, Age: {user.age}"
        }
    }

    // Display any errors
    if let Some(error) = user_bridge.get_error() {
        div { style: "color: red;", "User Bridge Error: {error}" }
    }
}

License

This project is licensed under the MIT License - see the LICENSE.md file for details.