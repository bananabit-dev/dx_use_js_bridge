// Test function to verify the JsBridge works correctly
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    
    #[test]
    fn test_callback_registration() {
        let received_data = Arc::new(Mutex::new(None));
        let received_data_clone = received_data.clone();
        
        // Register a callback
        register_callback("test_callback".to_string(), move |json: String| {
            let mut data = received_data_clone.lock().unwrap();
            *data = Some(json);
        });
        
        // Simulate receiving a message (this would normally come from JNI)
        // In a real scenario, this would be triggered by the JNI callback
        let callbacks = CALLBACKS.lock().unwrap();
        if let Some(callback) = callbacks.get("test_callback") {
            callback(r#"{"test": "data"}"#.to_string());
        }
        
        // Check that the callback was called
        let data = received_data.lock().unwrap();
        assert!(data.is_some());
        assert_eq!(data.as_ref().unwrap(), r#"{"test": "data"}"#);
    }
}
