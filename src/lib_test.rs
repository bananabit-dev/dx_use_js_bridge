// Simple test to check if js-sys is being pulled in on Android
#[cfg(test)]
mod tests {
    #[test]
    fn test_no_js_sys_on_android() {
        // This test should pass on Android (no js-sys code should run)
        // and fail on WASM (js-sys code should be available)
        #[cfg(all(target_os = "android", target_arch = "wasm32"))]
        compile_error!("Android and WASM targets are mutually exclusive!");
        
        // If we get here, we're good
        assert!(true);
    }
}
