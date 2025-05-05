use std::{env, path::PathBuf};

fn main() {
    // Always re-run when NETMAP_LOCATION is updated
    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-env-changed=NETMAP_LOCATION");
    println!("cargo:rerun-if-env-changed=DISABLE_NETMAP_KERNEL");

    // If we're in CI / userspace mode, skip any attempt to link or build the
    // in-kernel Netmap module.
    if env::var("DISABLE_NETMAP_KERNEL").is_ok() {
        println!("cargo:warning=DISABLE_NETMAP_KERNEL set; skipping Netmap kernel integration");
    } else {
        // Where did the user install netmap? Defaults to /usr/local
        let install_dir = env::var("NETMAP_LOCATION").unwrap_or_else(|_| "/usr/local".into());
        println!("cargo:warning=Linking against Netmap in: {}", install_dir);

        // Tell Cargo / rustc where to find the shared library
        println!("cargo:rustc-link-search=native={}/lib", install_dir);
        println!("cargo:rustc-link-lib=dylib=netmap");
    }

    // Now generate the Rust bindings via bindgen:
    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
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
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings to file");
}
