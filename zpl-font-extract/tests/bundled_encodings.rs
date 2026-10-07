//! Check the 2026-10-06 ZD621 203-dpi/V93.21.33Z snapshot against its portable JSON.
//! This is an archive regression, not evidence of a new printer capture.
use std::{fs, path::PathBuf};
use zpl_font_extract::collection::{compile, unhex, Collection};
#[path = "../../zpl-bitmap-fonts/src/zd621/fonts.rs"]
mod generated;

#[test]
fn bundled_encodings_match_json_and_regenerate_exactly() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../zpl-bitmap-fonts");
    let c = Collection::load(&root.join("data/zd621/fonts.json")).unwrap();
    assert_eq!(c.fonts.len(), 47);
    assert_eq!(
        c.fonts.iter().map(|f| f.records.len()).sum::<usize>(),
        10111
    );
    assert_eq!(
        c.coverage
            .iter()
            .map(|f| f.observed_equivalent_records)
            .sum::<usize>(),
        6872
    );
    assert_eq!(
        c.coverage
            .iter()
            .map(|f| f.unresolved_visible_record_ids.len())
            .sum::<usize>(),
        1290
    );
    assert_eq!(c.fonts.len(), generated::FONTS.len());
    for f in &c.fonts {
        let actual = generated::font_by_name(&f.name).unwrap();
        assert_eq!(actual.metrics, f.metrics);
        assert_eq!(
            actual.ids,
            f.records.iter().map(|r| r.id).collect::<Vec<_>>()
        );
        for r in &f.records {
            let g = actual.record(r.id).unwrap();
            assert_eq!(
                (g.advance, g.left, g.top, g.width, g.height),
                (r.advance, r.left, r.top, r.width, r.height)
            );
            assert_eq!(g.bitmap(), unhex(&r.bitmap_hex).unwrap());
            // Check declared-width access independently from stored row padding.
            let bits = unhex(&r.bitmap_hex).unwrap();
            for y in 0..r.height {
                for x in 0..r.width {
                    let bit = bits
                        [usize::from(y) * usize::from(r.width).div_ceil(8) + usize::from(x) / 8]
                        & (128 >> (x % 8))
                        != 0;
                    assert_eq!(g.pixel(x, y), bit);
                }
            }
        }
        assert_eq!(actual.encodings.len(), f.encodings.len());
        for (map, expected) in actual.encodings.iter().zip(&f.encodings) {
            assert_eq!(
                format!("{:?}", map.encoding),
                format!("{:?}", expected.encoding)
            );
            assert_eq!(
                map.inputs,
                expected.entries.iter().map(|e| e.input).collect::<Vec<_>>()
            );
            for entry in &expected.entries {
                let found = map.lookup(entry.input).unwrap();
                assert_eq!(format!("{:?}", found.status), format!("{:?}", entry.status));
                assert_eq!(found.candidates, entry.candidates);
            }
        }
    }
    assert_eq!(generated::CANDIDATE_CODE_PAGES.len(), 61);
    for (actual, expected) in generated::CANDIDATE_CODE_PAGES
        .iter()
        .zip(&c.candidate_code_pages)
    {
        assert_eq!(actual.ci, expected.ci);
        assert_eq!(
            actual.byte_to_character.as_slice(),
            expected.byte_to_character
        );
    }
    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let temp = Temp(std::env::temp_dir().join(format!(
            "bundled-encodings-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )));
    compile::compile(&c, &temp.0).unwrap();
    for name in ["fonts.rs", "bitmaps.bin", "catalog.json"] {
        let expected = fs::read(root.join("src/zd621").join(name)).unwrap();
        assert!(
            fs::read(temp.0.join(name)).unwrap() == expected,
            "generated {name} differs from checked-in file"
        );
    }
}
