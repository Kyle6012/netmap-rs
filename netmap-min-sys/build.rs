use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=wrapper.h");

    // tell cargo to look for shraed libraries in the following directories 
    println!("cargo:rust-link-search=/usr/local/lib");
    println!("cargo:rustc-link-lib=netmap");

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .allowlist_type("netmap_.*")
        .allowlist_function("nm_.*")
        .allowlist_var("NETMAP_.*")
        .size_t_is_usize(true)
        .default_enum_style(bindgen::EnumVariation::Rust {
            non_exhaustive:false,
        })
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings");
}
