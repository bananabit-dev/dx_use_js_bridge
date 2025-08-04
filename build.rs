fn main() {
    // Only for Android builds
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "android" {
        // Ensure we're linking against the correct libraries
        println!("cargo:rustc-link-lib=dylib=log");
        println!("cargo:rustc-link-lib=dylib=android");
        println!("cargo:rustc-link-lib=static=c++_shared");
        
        // Enable the uuid feature for Android builds
        println!("cargo:rustc-cfg=feature=\"uuid\"");
    }
}
