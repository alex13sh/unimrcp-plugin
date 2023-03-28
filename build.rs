fn main() {
    //    println!("cargo:rerun-if-changed=native/dgmrcp.h");

    // Links for testing
   println!("cargo:rerun-if-changed=native/dgmrcp.h");

    // Links for testing
    println!("cargo:rustc-link-lib=apr-1");
    println!("cargo:rustc-link-lib=unimrcpserver");
    println!("cargo:rustc-link-search=/opt/unimrcp/lib");
    println!("cargo:rustc-link-search=/usr/local/unimrcp/lib");
    println!("cargo:rustc-link-search=/usr/local/apr/lib");

    let mut builder = bindgen::Builder::default();
    builder = builder
        .header("includes/dgmrcp.h")
       .clang_arg("-I/usr/local/unimrcp/include")
        .clang_arg("-I/usr/local/apr/include/apr-1");
    let bindings = builder
        .constified_enum_module("*")
        .prepend_enum_name(false)
        // The problem with generating `FALSE`
        // it is generated not as `apt_bool_t` but as `u32`
        // so it had to be defined explicitly.
        .blocklist_item("FALSE")
        .derive_eq(true)
        .generate()
        .expect("Unable to generate bindings.");
    let out_path = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Unable to write bindings.");
    bindings.write_to_file("../bindings.rs").unwrap();
}
