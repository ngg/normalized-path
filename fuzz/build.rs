fn main() -> Result<(), std::env::VarError> {
    println!("cargo::rerun-if-changed=build.rs");

    if std::env::var("TARGET")? == "aarch64-pc-windows-msvc" {
        // Supply libFuzzer's coverage section markers without enabling ASan.
        println!("cargo::rustc-link-lib=static=clang_rt.asan_dynamic_runtime_thunk-aarch64");
    }

    Ok(())
}
