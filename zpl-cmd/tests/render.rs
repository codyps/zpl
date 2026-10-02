use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use zpl::{
    output::{Adapter, Pdf, Png, Svg},
    render::{profiles, render},
};

struct Workspace(PathBuf);

impl Workspace {
    fn new(source: &[u8]) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "zpl-cmd-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("input label.zpl"), source).unwrap();
        Self(path)
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_zpl-cmd"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .unwrap()
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn success(output: Output) -> Output {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[test]
fn help_version_and_usage_are_available_without_input_files() {
    let workspace = Workspace::new(b"");
    let help = success(workspace.run(&["--help"]));
    assert!(String::from_utf8_lossy(&help.stdout).contains("render"));
    let help = success(workspace.run(&["render", "--help"]));
    let help = String::from_utf8_lossy(&help.stdout);
    for expected in [
        "<INPUT>",
        "<OUTPUT>",
        ".pdf",
        "--profile",
        "--explicit-qr-mask",
    ] {
        assert!(help.contains(expected), "{expected}: {help}");
    }
    let version = success(workspace.run(&["--version"]));
    assert_eq!(
        String::from_utf8(version.stdout).unwrap().trim(),
        format!("zpl-cmd {}", env!("CARGO_PKG_VERSION"))
    );
    for args in [
        vec![],
        vec!["unknown"],
        vec!["render"],
        vec!["render", "input label.zpl"],
        vec![
            "render",
            "input label.zpl",
            "out.pdf",
            "--profile",
            "unknown",
        ],
        vec!["render", "input label.zpl", "out.pdf", "--profile"],
        vec!["render", "input label.zpl", "out.pdf", "--unknown"],
    ] {
        let result = workspace.run(&args);
        assert_eq!(result.status.code(), Some(2), "{args:?}");
        assert!(!result.stderr.is_empty());
        assert!(!workspace.0.join("out.pdf").exists());
    }
}

#[test]
fn renders_all_formats_using_the_default_profile() {
    let source = b"^XA^PW160^LL100^FO10,10^AAN,18,10^FDCLI^FS^FO10,40^GB40,20,3^FS^XZ";
    let workspace = Workspace::new(source);
    let document = render(source, profiles::ZD621_203_DPI).unwrap();
    for (extension, expected) in [
        ("png", Png.encode(&document.labels[0]).unwrap()),
        ("svg", Svg.encode(&document.labels[0]).unwrap()),
        ("pdf", Pdf.encode_pages(&document.labels).unwrap()),
    ] {
        let output = format!("output label.{extension}");
        let result = success(workspace.run(&["render", "input label.zpl", &output]));
        assert!(result.stdout.is_empty());
        assert_eq!(fs::read(workspace.0.join(output)).unwrap(), expected);
    }
}

#[test]
fn pdf_supports_multiple_labels_and_images_reject_them_before_writing() {
    let source = b"^XA^PW100^LL80^XZ^XA^PW80^LL100^FO10,10^GB20,20,20^FS^XZ";
    let workspace = Workspace::new(source);
    let document = render(source, profiles::SPECIFICATION).unwrap();
    success(workspace.run(&[
        "render",
        "input label.zpl",
        "out.pdf",
        "--profile",
        "specification",
    ]));
    assert_eq!(
        fs::read(workspace.0.join("out.pdf")).unwrap(),
        Pdf.encode_pages(&document.labels).unwrap()
    );
    for output in ["out.png", "out.svg"] {
        fs::write(workspace.0.join(output), b"keep existing output").unwrap();
        let result = workspace.run(&["render", "input label.zpl", output]);
        assert_eq!(result.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&result.stderr).contains("exactly one label"));
        assert_eq!(
            fs::read(workspace.0.join(output)).unwrap(),
            b"keep existing output"
        );
    }
}

#[test]
fn profile_and_explicit_mask_options_match_the_library_in_either_order() {
    // QR placement/masks follow the same profile options as the former CLI.
    // Zebra ^BQ/^BY: docs/barcodes.md; capture evidence: docs/printer-accuracy.md.
    let source = b"^XA^PW150^LL120^FO10,10^BQN,2,2,Q,0^FDQA,CLI^FS^XZ";
    let workspace = Workspace::new(source);
    let mut preview = profiles::ZD621_203_DPI;
    preview.compatibility.preview_width_quantum = Some(64);
    preview.compatibility.preview_width_latched_at_first_draw = true;
    for (name, base) in [
        ("zd621", profiles::ZD621_203_DPI),
        ("zd621-preview", preview),
        ("zq610-plus", profiles::ZQ610_PLUS_203_DPI),
        ("specification", profiles::SPECIFICATION),
    ] {
        for explicit in [false, true] {
            let mut options = base;
            if explicit {
                options.compatibility.qr_printer_mask_selection = false;
            }
            let document = render(source, options).unwrap();
            let expected = Png.encode(&document.labels[0]).unwrap();
            let mut args = vec!["render", "--profile", name, "input label.zpl", "out.png"];
            if explicit {
                args.insert(1, "--explicit-qr-mask");
            }
            success(workspace.run(&args));
            assert_eq!(
                fs::read(workspace.0.join("out.png")).unwrap(),
                expected,
                "{args:?}"
            );
            if explicit {
                args.remove(1);
                args.push("--explicit-qr-mask");
                success(workspace.run(&args));
                assert_eq!(
                    fs::read(workspace.0.join("out.png")).unwrap(),
                    expected,
                    "{args:?}"
                );
            }
        }
    }
}

#[test]
fn rendering_and_io_errors_report_failure_without_replacing_existing_output() {
    let workspace = Workspace::new(b"^XA^FO10,10^GB20,20,20^FS^XZ");
    let destination = workspace.0.join("out.pdf");
    fs::write(&destination, b"keep existing output").unwrap();
    let result = workspace.run(&["render", "missing.zpl", "out.pdf"]);
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).contains("reading missing.zpl"));
    assert_eq!(fs::read(&destination).unwrap(), b"keep existing output");

    let result = workspace.run(&["render", "input label.zpl", "missing/out.pdf"]);
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).contains("writing missing/out.pdf"));

    let unsupported = workspace.0.join("out.jpg");
    fs::write(&unsupported, b"keep existing output").unwrap();
    let result = workspace.run(&["render", "input label.zpl", "out.jpg"]);
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).contains("output extension"));
    assert_eq!(fs::read(unsupported).unwrap(), b"keep existing output");

    fs::write(workspace.0.join("input label.zpl"), b"^XA").unwrap();
    let result = workspace.run(&["render", "input label.zpl", "out.pdf"]);
    assert_eq!(result.status.code(), Some(1));
    assert!(!result.stderr.is_empty());
    assert_eq!(fs::read(destination).unwrap(), b"keep existing output");
}
