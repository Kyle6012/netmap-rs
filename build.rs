use std::env;
use std::path::Path;

fn main() {
    // Check if we're building with sys feature enabled
    if env::var_os("CARGO_FEATURE_SYS").is_some() {
        // Try to find netmap installation
        let netmap_location = env::var("NETMAP_LOCATION")
            .unwrap_or_else(|_| "/usr/local".to_string());
        
        let include_path = Path::new(&netmap_location).join("include");
        let lib_path = Path::new(&netmap_location).join("lib");
        
        // Check for netmap headers
        let netmap_user_h = include_path.join("net").join("netmap_user.h");
        let netmap_h = include_path.join("net").join("netmap.h");
        
        if netmap_user_h.exists() && netmap_h.exists() {
            println!("cargo:rustc-cfg=netmap_available");
            println!("cargo:rustc-link-search=native={}", lib_path.display());
            println!("cargo:rustc-link-lib=static=netmap");
            println!("cargo:include={}", include_path.display());
            
            // Add include path for bindgen
            println!("cargo:rustc-env=NETMAP_INCLUDE_PATH={}", include_path.display());
        } else {
            println!("cargo:warning=Netmap headers not found at {}. Building without netmap support.", netmap_location);
            println!("cargo:warning=To specify netmap location, set NETMAP_LOCATION environment variable");
            println!("cargo:warning=Example: NETMAP_LOCATION=/opt/netmap cargo build");
        }
    }
    
    // Re-run if environment variables change
    println!("cargo:rerun-if-env-changed=NETMAP_LOCATION");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_SYS");
}