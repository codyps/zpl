use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct TestConfig {
    prefix: String,
}

fn run_test(name: &str) {
    let dir = std::path::Path::new("../test-data");

    let test_zpl_path = dir.join(name);

    let mut test_toml_path = test_zpl_path.clone();
    test_toml_path.set_extension(".toml");

    let test_zpl = std::fs::read(test_zpl_path).unwrap();
    let test_toml_maybe = match std::fs::read(test_toml_path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
        Ok(v) => Ok(Some(v)),
    }
    .unwrap();
}

#[test]
fn cc2() {
    run_test("cc2.zpl");
}
