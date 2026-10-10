//! Compile and execute real C/C++ consumers against Cargo's shared library.
//! Cargo integration tests build the library's declared crate types before this
//! harness: https://doc.rust-lang.org/cargo/reference/cargo-targets.html#tests
//! cc discovers MSVC and its SDK environment as well as Unix compilers:
//! https://docs.rs/cc/latest/cc/struct.Build.html#method.get_compiler
use std::{
    path::{Path, PathBuf},
    process::Command,
};

fn run(command: &mut Command) {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("{command:?}: {error}"));
    assert!(
        output.status.success(),
        "{command:?} exited with {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn check_consumer(source: &str, cpp: bool, example: bool) {
    // Referencing the Rust library also makes the native library build dependency
    // explicit. No nested Cargo invocation or shared-target lock is needed.
    assert_eq!(zpl_c::zpl_abi_version(), 3);
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let deps = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let temp = tempfile::tempdir().unwrap();
    let msvc = env!("ZPL_C_TEST_TARGET").ends_with("-msvc");
    let (dynamic, link) = if cfg!(target_os = "windows") {
        (
            "zpl_c.dll",
            if msvc {
                "zpl_c.dll.lib"
            } else {
                "libzpl_c.dll.a"
            },
        )
    } else if cfg!(target_os = "macos") {
        ("libzpl_c.dylib", "libzpl_c.dylib")
    } else {
        ("libzpl_c.so", "libzpl_c.so")
    };
    let library = deps.join(link);
    assert!(
        library.is_file(),
        "Cargo did not build {}",
        library.display()
    );
    let executable: PathBuf = temp.path().join(if cfg!(windows) {
        "consumer.exe"
    } else {
        "consumer"
    });
    let compiler = cc::Build::new()
        .host(env!("ZPL_C_TEST_HOST"))
        .target(env!("ZPL_C_TEST_TARGET"))
        .opt_level(0)
        .debug(false)
        .cargo_metadata(false)
        .cpp(cpp)
        .get_compiler();
    let mut command = compiler.to_command();
    command.current_dir(temp.path());
    if compiler.is_like_msvc() {
        // C11 and C++14 are the earliest explicit language switches MSVC offers.
        // fopen is portable C; do not replace it with the Microsoft-only fopen_s.
        command
            .args([
                "/nologo",
                "/W4",
                "/WX",
                "/UNDEBUG",
                "/D_CRT_SECURE_NO_WARNINGS",
            ])
            .arg(if cpp { "/TP" } else { "/TC" })
            .arg(if cpp { "/std:c++14" } else { "/std:c11" })
            .arg(format!("/I{}", crate_dir.join("include").display()))
            .arg(crate_dir.join(source))
            .arg(format!("/Fe{}", executable.display()))
            .arg("/link")
            .arg(&library);
    } else {
        command
            .args(["-Wall", "-Wextra", "-Werror", "-pedantic", "-UNDEBUG"])
            .arg(if cpp { "-std=c++11" } else { "-std=c11" })
            .arg("-I")
            .arg(crate_dir.join("include"))
            .args(["-x", if cpp { "c++" } else { "c" }])
            .arg(crate_dir.join(source))
            .args(["-x", "none"])
            .arg(&library)
            .arg("-o")
            .arg(&executable);
        if !cfg!(windows) {
            command.arg(format!("-Wl,-rpath,{}", deps.display()));
        }
    }
    run(&mut command);
    if cfg!(windows) {
        // Windows loads dependencies beside the executable, independent of PATH
        // or the working directory (the font test reads repository fixtures).
        std::fs::copy(deps.join(dynamic), temp.path().join(dynamic)).unwrap();
    }
    run(Command::new(&executable).current_dir(if example {
        temp.path()
    } else {
        crate_dir.parent().unwrap()
    }));
    if example {
        let png = std::fs::read(temp.path().join("label.png")).unwrap();
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
    }
}

#[test]
fn c_smoke() {
    check_consumer("tests/smoke.c", false, false);
}
#[test]
fn cpp_smoke() {
    check_consumer("tests/smoke.c", true, false);
}
#[test]
fn c_fonts() {
    check_consumer("tests/fonts.c", false, false);
}
#[test]
fn cpp_fonts() {
    check_consumer("tests/fonts.c", true, false);
}
#[test]
fn c_render_example() {
    check_consumer("examples/render.c", false, true);
}
