use std::env;
use std::path::PathBuf;

fn main() {
    // Only for Android builds
    if env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "android" {
        // Get the Android NDK path
        let ndk_path = env::var("ANDROID_NDK_HOME")
            .or_else(|_| env::var("NDK_HOME"))
            .unwrap_or_else(|_| {
                // Default NDK path
                format!("{}/Android/Sdk/ndk/29.0.13599879", env::var("HOME").unwrap_or_default())
            });
        
        let ndk_path_buf = PathBuf::from(ndk_path);
        
        // Add the NDK sysroot to the linker search path
        let sysroot = ndk_path_buf.join("toolchains/llvm/prebuilt/linux-x86_64/sysroot");
        if sysroot.exists() {
            println!("cargo:rustc-link-search=native={}/usr/lib/aarch64-linux-android", sysroot.display());
            println!("cargo:rustc-link-search=native={}/usr/lib/arm-linux-androideabi", sysroot.display());
            println!("cargo:rustc-link-search=native={}/usr/lib/x86_64-linux-android", sysroot.display());
            println!("cargo:rustc-link-search=native={}/usr/lib/i686-linux-android", sysroot.display());
        }
        
        // Ensure we're linking against the correct libraries
        println!("cargo:rustc-link-lib=dylib=log");
        println!("cargo:rustc-link-lib=dylib=android");
        println!("cargo:rustc-link-lib=dylib=c++_shared");
        
        // Add the NDK lib directories to search paths
        if let Ok(target) = env::var("TARGET") {
            let lib_dir = match target.as_str() {
                "aarch64-linux-android" => "aarch64-linux-android",
                "armv7-linux-androideabi" => "arm-linux-androideabi",
                "x86_64-linux-android" => "x86_64-linux-android",
                "i686-linux-android" => "i686-linux-android",
                _ => "aarch64-linux-android", // Default
            };
            
            let lib_path = sysroot.join(format!("usr/lib/{}", lib_dir));
            if lib_path.exists() {
                println!("cargo:rustc-link-search=native={}", lib_path.display());
            }
        }
        
        // Enable the uuid feature for Android builds
        println!("cargo:rustc-cfg=feature=\"uuid\"");
    }
}
