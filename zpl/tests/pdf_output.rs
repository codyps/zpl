//! Independent parsing checks against ISO 32000-1:2008 §§7.5, 7.7, 8.3, 8.5,
//! and 11.3.5 (https://pdfa.org/resource/pdf-specification-archive/).
use lopdf::{content::Content, Document, Object, ObjectId};
use zpl::{
    output::{Adapter, Draw, Paint, Path, Pdf, Point, Scene, Segment, MAX_SEGMENTS},
    render::{profiles::SPECIFICATION, render},
};

fn number(object: &Object) -> f64 {
    match object {
        Object::Integer(value) => *value as f64,
        Object::Real(value) => f64::from(*value),
        _ => panic!("expected a PDF number: {object:?}"),
    }
}

fn page_size(doc: &Document, id: ObjectId) -> (f64, f64) {
    let page = doc.get_dictionary(id).unwrap();
    let media = page.get(b"MediaBox").unwrap().as_array().unwrap();
    let unit = page.get(b"UserUnit").map(number).unwrap_or(1.0);
    (number(&media[2]) * unit, number(&media[3]) * unit)
}

#[test]
fn pages_preserve_order_physical_size_and_independent_content() {
    let scenes = [
        Scene::new(812, 1218, 203).unwrap(),
        Scene::new(600, 300, 300).unwrap(),
        Scene::new(406, 203, 203).unwrap(),
    ];
    let bytes = Pdf.encode_pages(&scenes).unwrap();
    let doc = Document::load_mem(&bytes).unwrap();
    assert_eq!(doc.version, "1.7");
    let pages = doc.get_pages();
    assert_eq!(pages.len(), 3);
    for ((_, id), (width, height)) in pages.iter().zip([(288., 432.), (144., 72.), (144., 72.)]) {
        assert_eq!(page_size(&doc, *id), (width, height));
        let content = Content::decode(&doc.get_page_content(*id)).unwrap();
        let transform = &content.operations[1];
        assert_eq!(transform.operator, "cm");
        assert!(number(&transform.operands[0]) > 0.);
        assert!(number(&transform.operands[3]) < 0.);
        assert_eq!(number(&transform.operands[5]), height);
    }
    assert_eq!(
        Pdf.encode(&scenes[0]).unwrap(),
        Pdf.encode_pages(&scenes[..1]).unwrap()
    );
    assert_eq!(bytes, Pdf.encode_pages(&scenes).unwrap());
}

#[test]
fn large_pages_use_user_units_without_resizing_the_label() {
    // 400 inches is outside the default 200-inch PDF page limit (Annex C).
    let scene = Scene::new(400, 200, 1).unwrap();
    let doc = Document::load_mem(&Pdf.encode(&scene).unwrap()).unwrap();
    let id = doc.get_pages()[&1];
    assert_eq!(page_size(&doc, id), (28_800., 14_400.));
    let page = doc.get_dictionary(id).unwrap();
    assert_eq!(number(page.get(b"UserUnit").unwrap()), 2.);
    assert_eq!(
        page.get(b"MediaBox")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(number)
            .collect::<Vec<_>>(),
        [0., 0., 14_400., 7200.]
    );
}

#[test]
fn fractional_dimensions_and_high_dpi_keep_nonzero_transforms() {
    for dpi in [203, 300, 600, 1200, u32::MAX] {
        let scene = Scene::new(101, 53, dpi).unwrap();
        let doc = Document::load_mem(&Pdf.encode(&scene).unwrap()).unwrap();
        let id = doc.get_pages()[&1];
        let (w, h) = page_size(&doc, id);
        assert!((w - 101. * 72. / f64::from(dpi)).abs() < 0.00001);
        assert!((h - 53. * 72. / f64::from(dpi)).abs() < 0.00001);
        let content = Content::decode(&doc.get_page_content(id)).unwrap();
        assert!(number(&content.operations[1].operands[0]) > 0.);
    }
}

#[test]
fn paths_clipping_and_inversion_are_vector_pdf_operations() {
    let mut scene = Scene::new(80, 60, 203).unwrap();
    let mut path = Path::default();
    path.rect(-10., 5., 40., 20.);
    path.rect(0., 10., 5., 5.); // Even-odd hole, not a separate white draw.
    let mut curve = Path::default();
    curve.ellipse(10., 10., 20., 30.);
    for (path, paint) in [
        (path.clone(), Paint::Black),
        (curve, Paint::Invert),
        (path.clone(), Paint::White),
        (path, Paint::Black),
    ] {
        scene.draws.push(Draw { path, paint });
    }
    let doc = Document::load_mem(&Pdf.encode(&scene).unwrap()).unwrap();
    let id = doc.get_pages()[&1];
    let page = doc.get_dictionary(id).unwrap();
    let resources = page.get(b"Resources").unwrap().as_dict().unwrap();
    assert!(!resources.has(b"Font"));
    assert!(!resources.has(b"XObject"));
    let invert = resources
        .get(b"ExtGState")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"Invert")
        .unwrap()
        .as_dict()
        .unwrap();
    assert_eq!(invert.get(b"BM").unwrap().as_name().unwrap(), b"Difference");
    let group = page.get(b"Group").unwrap().as_dict().unwrap();
    assert!(group.get(b"I").unwrap().as_bool().unwrap());
    assert_eq!(group.get(b"CS").unwrap().as_name().unwrap(), b"DeviceGray");
    let content = Content::decode(&doc.get_page_content(id)).unwrap();
    let ops = &content.operations;
    let operators: Vec<_> = ops.iter().map(|op| op.operator.as_str()).collect();
    assert_eq!(&operators[..7], ["q", "cm", "re", "W", "n", "g", "re"]);
    assert_eq!(operators.iter().filter(|&&op| op == "f*").count(), 4);
    assert_eq!(operators.iter().filter(|&&op| op == "c").count(), 4);
    assert_eq!(operators.iter().filter(|&&op| op == "q").count(), 5);
    assert_eq!(operators.iter().filter(|&&op| op == "Q").count(), 5);
    let paints: Vec<_> = ops
        .iter()
        .filter(|op| op.operator == "g")
        .map(|op| number(&op.operands[0]))
        .collect();
    assert_eq!(paints, [1., 0., 1., 1., 0.]); // Backdrop and ordered paints.
    let inversion = operators.iter().position(|&op| op == "gs").unwrap();
    assert_eq!(operators[inversion - 1], "q");
    assert_eq!(ops[inversion].operands[0].as_name().unwrap(), b"Invert");
}

#[test]
fn cross_reference_offsets_and_stream_lengths_are_exact() {
    let scenes = vec![Scene::new(20, 30, 203).unwrap(); 64];
    let bytes = Pdf.encode_pages(&scenes).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    let startxref: usize = text
        .split("startxref\n")
        .nth(1)
        .unwrap()
        .lines()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(bytes[startxref..].starts_with(b"xref\n"));
    let mut lines = std::str::from_utf8(&bytes[startxref..]).unwrap().lines();
    assert_eq!(lines.next(), Some("xref"));
    assert_eq!(lines.next(), Some("0 131"));
    assert_eq!(lines.next(), Some("0000000000 65535 f "));
    for (index, line) in lines.take(130).enumerate() {
        assert_eq!(line.len(), 19);
        let offset: usize = line[..10].parse().unwrap();
        assert!(bytes[offset..].starts_with(format!("{} 0 obj\n", index + 1).as_bytes()));
    }
    let doc = Document::load_mem(&bytes).unwrap();
    assert_eq!(doc.get_pages().len(), 64);
    for object in doc.objects.values() {
        if let Object::Stream(stream) = object {
            assert_eq!(
                stream.dict.get(b"Length").unwrap().as_i64().unwrap() as usize,
                stream.content.len()
            );
        }
    }
}

#[test]
fn rejects_invalid_scenes_and_document_limits() {
    let scene = Scene::new(20, 30, 203).unwrap();
    assert!(Pdf.encode_pages(&[]).is_err());
    assert!(Pdf.encode_pages(&vec![scene.clone(); 65]).is_err());
    for (width, height, dpi) in [(0, 30, 203), (20, 0, 203), (20, 30, 0), (u32::MAX, 30, 203)] {
        let bad = Scene {
            width,
            height,
            dpi,
            draws: vec![],
        };
        assert!(Pdf.encode_pages(&[scene.clone(), bad]).is_err());
    }
    for segments in [
        vec![Segment::Line(Point::new(1., 2.))],
        vec![Segment::Move(Point::new(f64::NAN, 0.))],
        vec![Segment::Move(Point::new(0., f64::INFINITY))],
        vec![Segment::Move(Point::new(1e10, 0.))],
    ] {
        let mut bad = scene.clone();
        bad.draws.push(Draw {
            path: Path { segments },
            paint: Paint::Black,
        });
        assert!(Pdf.encode(&bad).is_err());
    }
    let mut many = scene.clone();
    many.draws.push(Draw {
        path: Path {
            segments: vec![Segment::Move(Point::new(0., 0.)); MAX_SEGMENTS / 2 + 1],
        },
        paint: Paint::Black,
    });
    assert!(Pdf.encode_pages(&[many.clone(), many]).is_err());
    assert!(Pdf.encode(&Scene::new(20_000_000, 1, 1).unwrap()).is_err());
}

#[test]
fn rendered_zpl_produces_one_pdf_page_per_label() {
    let doc = render(
        b"^XA^PW400^LL200^FO10,10^AAN,18,10^FDHello^FS^FO10,40^BQN,2,3^FDQA,PDF^FS^XZ\
          ^XA^PW203^LL406^FO10,10^GB50,50,50^FS^FO30,30^FR^GB50,50,50^FS^XZ",
        SPECIFICATION,
    )
    .unwrap();
    let pdf = Document::load_mem(&Pdf.encode_pages(&doc.labels).unwrap()).unwrap();
    assert_eq!(pdf.get_pages().len(), 2);
    for (scene, id) in doc.labels.iter().zip(pdf.get_pages().values()) {
        let content = Content::decode(&pdf.get_page_content(*id)).unwrap();
        assert_eq!(
            content
                .operations
                .iter()
                .filter(|op| op.operator == "f*")
                .count(),
            scene.draws.len()
        );
        assert!(!scene.draws.is_empty());
    }
}

#[test]
fn configured_pdf_limits_allow_more_pages_and_coordinates() {
    use zpl::output::Limits;
    let mut scene = Scene::new(20, 30, 203).unwrap();
    scene.draws.push(Draw {
        path: Path {
            segments: vec![Segment::Move(Point::new(1e10, 0.))],
        },
        paint: Paint::Black,
    });
    let scenes = vec![scene; 65];
    let bytes = Pdf
        .encode_pages_with_limits(&scenes, Limits::unlimited())
        .unwrap();
    let doc = Document::load_mem(&bytes).unwrap();
    assert_eq!(doc.get_pages().len(), 65);
    for limits in [
        Limits {
            pages: 64,
            ..Limits::unlimited()
        },
        Limits {
            segments: 64,
            ..Limits::unlimited()
        },
        Limits {
            coordinate_abs: 1e9,
            ..Limits::unlimited()
        },
    ] {
        assert!(Pdf.encode_pages_with_limits(&scenes, limits).is_err());
    }
    assert!(Pdf
        .encode_pages_with_limits(&[], Limits::unlimited())
        .is_err());
    // ISO 32000-1 Annex C's UserUnit ceiling remains a format constraint.
    let huge = Scene::new_with_limits(u32::MAX, 1, 1, Limits::unlimited()).unwrap();
    assert!(Pdf
        .encode_pages_with_limits(&[huge], Limits::unlimited())
        .is_err());
}
