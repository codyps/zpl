fn main() {
    // cc needs the actual build triples when invoked by the integration harness,
    // where Cargo no longer supplies build-script HOST/TARGET variables.
    for key in ["HOST", "TARGET"] {
        println!(
            "cargo:rustc-env=ZPL_C_TEST_{key}={}",
            std::env::var(key).unwrap()
        );
    }
    println!("cargo:rerun-if-changed=build.rs");
}
