use std::path::Path;
use zpl::parse::{Element, ParseContext};

#[test]
fn repository_fixtures_have_expected_commands_and_round_trip() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../test-data");
    let expected = [
        ("bz.zpl", 7, b'^'),
        ("cc.zpl", 3, b'/'),
        ("cc2.zpl", 4, b'/'),
        // This historical fixture includes a shell wrapper and lowercase text.
        ("hello-world.zpl", 6, b'^'),
    ];
    for (name, command_count, final_prefix) in expected {
        let input = std::fs::read(dir.join(name)).unwrap();
        let mut parser = ParseContext::from_bytes(&input);
        let parts = parser.by_ref().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(
            parts
                .iter()
                .filter(|p| !matches!(p, Element::BeforeFirstCommand(_)))
                .count(),
            command_count,
            "{name}"
        );
        assert_eq!(parser.syntax().format_prefix, final_prefix, "{name}");
        let restored: Vec<_> = parts
            .iter()
            .flat_map(|p| p.as_bytes().iter().copied())
            .collect();
        assert_eq!(restored, input, "{name}");
    }
    let discovered = std::fs::read_dir(&dir)
        .unwrap()
        .filter(|entry| {
            entry
                .as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|ext| ext == "zpl")
        })
        .count();
    assert_eq!(
        discovered,
        expected.len(),
        "add expectations for new ZPL fixtures"
    );
}
