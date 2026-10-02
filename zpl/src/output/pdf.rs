//! Original PDF writer for scene paths; no fonts or raster images are embedded.
//!
//! ISO 32000-1:2008, §§7.3–7.7 (objects, streams, xref and pages),
//! 8.3.2 (coordinates), 8.5.2–8.5.3 (paths and even-odd fill),
//! 11.3.5/11.6.6 (Difference blending and page groups), and Annex C (limits).
//! <https://pdfa.org/resource/pdf-specification-archive/>

use std::{fmt::Write as _, io::Write as _};

use super::{Adapter, OutputError, Paint, Scene, Segment, MAX_SEGMENTS};

/// Vector PDF 1.7 output, with label dimensions converted from dots to inches
/// using each scene's DPI. Text retains the renderer's glyph paths.
#[derive(Debug, Default, Clone, Copy)]
pub struct Pdf;

impl Adapter for Pdf {
    fn encode(&self, scene: &Scene) -> Result<Vec<u8>, OutputError> {
        self.encode_pages(std::slice::from_ref(scene))
    }
}

impl Pdf {
    /// Encode one scene per page, in order, allowing different sizes and DPIs.
    ///
    /// Requires 1–64 scenes and at most [`MAX_SEGMENTS`] segments in total.
    /// All scenes are validated before encoding. Oversized physical pages use
    /// PDF's `UserUnit`, up to its 75,000 limit; larger pages return an error.
    /// Output is deterministic and contains no timestamps or source ZPL.
    pub fn encode_pages(&self, scenes: &[Scene]) -> Result<Vec<u8>, OutputError> {
        if scenes.is_empty() || scenes.len() > 64 {
            return Err(OutputError("PDF requires between 1 and 64 pages"));
        }
        let mut segments = 0usize;
        let mut sizes = Vec::with_capacity(scenes.len());
        for scene in scenes {
            scene.validate()?;
            for draw in &scene.draws {
                segments = segments
                    .checked_add(draw.path.segments.len())
                    .filter(|&n| n <= MAX_SEGMENTS)
                    .ok_or(OutputError("too many PDF path segments"))?;
            }
            sizes.push(PageSize::new(scene)?);
        }

        let mut out = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n".to_vec();
        let mut offsets = Vec::new();
        // Objects 1 and 2 are the catalog and page tree; each page then owns
        // two consecutive objects (page dictionary and content stream).
        object(
            &mut out,
            &mut offsets,
            b"<< /Type /Catalog /Pages 2 0 R /ViewerPreferences << /PrintScaling /None >> >>",
        );
        let mut pages = format!("<< /Type /Pages /Count {} /Kids [", scenes.len());
        for i in 0..scenes.len() {
            write!(pages, "{} 0 R ", 3 + 2 * i).unwrap();
        }
        pages.push_str("] >>");
        object(&mut out, &mut offsets, pages.as_bytes());

        for (i, (scene, size)) in scenes.iter().zip(sizes).enumerate() {
            let page = format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w:.12} {h:.12}] \
                 /UserUnit {unit} /Resources << /ExtGState << \
                 /Invert << /Type /ExtGState /BM /Difference >> >> >> \
                 /Group << /S /Transparency /CS /DeviceGray /I true >> \
                 /Contents {content} 0 R >>",
                w = size.width,
                h = size.height,
                unit = size.unit,
                content = 4 + 2 * i,
            );
            object(&mut out, &mut offsets, page.as_bytes());

            let content = page_content(scene, &size);
            offsets.push(out.len());
            writeln!(
                out,
                "{} 0 obj\n<< /Length {} >>\nstream",
                offsets.len(),
                content.len()
            )
            .unwrap();
            out.extend_from_slice(content.as_bytes());
            out.extend_from_slice(b"\nendstream\nendobj\n");
        }

        let startxref = out.len();
        // Classic xref entries have exactly ten offset digits and twenty bytes.
        if startxref as u64 > 9_999_999_999 {
            return Err(OutputError("PDF exceeds cross-reference offset limit"));
        }
        writeln!(out, "xref\n0 {}\n0000000000 65535 f ", offsets.len() + 1).unwrap();
        for offset in &offsets {
            writeln!(out, "{offset:010} 00000 n ").unwrap();
        }
        write!(
            out,
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{startxref}\n%%EOF\n",
            offsets.len() + 1
        )
        .unwrap();
        Ok(out)
    }
}

struct PageSize {
    width: f64,
    height: f64,
    scale: f64,
    unit: f64,
}

impl PageSize {
    fn new(scene: &Scene) -> Result<Self, OutputError> {
        // A PDF default user unit is 1/72 inch. Keep MediaBox coordinates within
        // 14,400 units even at low DPI, without changing the physical label size.
        let points_per_dot = 72.0 / f64::from(scene.dpi);
        let longest = f64::from(scene.width.max(scene.height)) * points_per_dot;
        let unit = (longest / 14_400.0).ceil().max(1.0);
        if unit > 75_000.0 {
            return Err(OutputError("PDF page exceeds physical size limit"));
        }
        let scale = points_per_dot / unit;
        Ok(Self {
            width: f64::from(scene.width) * scale,
            height: f64::from(scene.height) * scale,
            scale,
            unit,
        })
    }
}

fn object(out: &mut Vec<u8>, offsets: &mut Vec<usize>, body: &[u8]) {
    offsets.push(out.len());
    writeln!(out, "{} 0 obj", offsets.len()).unwrap();
    out.extend_from_slice(body);
    out.extend_from_slice(b"\nendobj\n");
}

fn page_content(scene: &Scene, size: &PageSize) -> String {
    // Flip PDF's bottom-left origin into the scene's top-left origin, clip to
    // the label, then paint an opaque white backdrop for ordered inversion.
    let mut out = format!(
        "q\n{s:.12} 0 0 -{s:.12} 0 {h:.12} cm\n0 0 {w} {dots_h} re W n\n\
         1 g\n0 0 {w} {dots_h} re f\n",
        s = size.scale,
        h = size.height,
        w = scene.width,
        dots_h = scene.height,
    );
    for draw in &scene.draws {
        // Restore the graphics state after every draw so Difference cannot
        // leak into a following black or white paint operation.
        out.push_str("q\n");
        out.push_str(match draw.paint {
            Paint::Black => "0 g\n",
            Paint::White => "1 g\n",
            Paint::Invert => "/Invert gs\n1 g\n",
        });
        for segment in &draw.path.segments {
            match segment {
                Segment::Move(p) => writeln!(out, "{:.6} {:.6} m", p.x, p.y).unwrap(),
                Segment::Line(p) => writeln!(out, "{:.6} {:.6} l", p.x, p.y).unwrap(),
                Segment::Cubic(a, b, c) => writeln!(
                    out,
                    "{:.6} {:.6} {:.6} {:.6} {:.6} {:.6} c",
                    a.x, a.y, b.x, b.y, c.x, c.y
                )
                .unwrap(),
                Segment::Close => out.push_str("h\n"),
            }
        }
        out.push_str("f*\nQ\n");
    }
    out.push_str("Q\n");
    out
}
