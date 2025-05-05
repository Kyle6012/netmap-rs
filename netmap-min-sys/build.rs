use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-env-changed=NETMAP_LOCATION");
    println!("cargo:rerun-if-env-changed=DISABLE_NETMAP_KERNEL");

    // If we're disabling Netmap integration (macOS, Windows),
    // just emit an empty bindings.rs and exit early.
    if env::var("DISABLE_NETMAP_KERNEL").is_ok() {
        let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
        fs::write(out_path.join("bindings.rs"), "// empty, Netmap disabled\n")
            .expect("Failed to write empty bindings.rs");
        println!("cargo:warning=DISABLE_NETMAP_KERNEL set; skipping bindgen");
        return;
    }

    // Otherwise, proceed to link & bindgen:
    let install_dir = env::var("NETMAP_LOCATION").unwrap_or_else(|_| "/usr/local".into());
    println!("cargo:warning=Linking against Netmap in: {}", install_dir);
    println!("cargo:rustc-link-search=native={}/lib", install_dir);
    println!("cargo:rustc-link-lib=dylib=netmap");

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .allowlist_type("netmap_.*")
        .allowlist_function("nm_.*")
        .allowlist_var("NETMAP_.*")
        .size_t_is_usize(true)
        .default_enum_style(bindgen::EnumVariation::Rust {
            non_exhaustive: false,
        })
        .generate()
        .expect("Unable to generate bindings with bindgen");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("binding.rs"))
        .expect("Couldn't write bindings to file");
}
