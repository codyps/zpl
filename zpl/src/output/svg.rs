use super::*;
use std::fmt::Write;
#[derive(Debug, Default, Clone, Copy)]
pub struct Svg;
impl Adapter for Svg {
    fn encode(&self, scene: &Scene) -> Result<Vec<u8>, OutputError> {
        self.encode_with_limits(scene, Limits::default())
    }
}
impl Svg {
    /// Encode with caller-selected output budgets.
    pub fn encode_with_limits(
        &self,
        scene: &Scene,
        limits: Limits,
    ) -> Result<Vec<u8>, OutputError> {
        scene.validate_with_limits(limits)?;
        let mut out=format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\"><g style=\"isolation:isolate\"><rect width=\"100%\" height=\"100%\" fill=\"white\"/>",scene.width,scene.height,scene.width,scene.height);
        for draw in &scene.draws {
            let (color, style) = match draw.paint {
                Paint::Black => ("black", ""),
                Paint::White => ("white", ""),
                Paint::Invert => ("white", " style=\"mix-blend-mode:difference\""),
            };
            write!(
                out,
                "<path fill=\"{color}\" fill-rule=\"evenodd\"{style} d=\""
            )
            .unwrap();
            for segment in &draw.path.segments {
                match segment {
                    Segment::Move(p) => write!(out, "M{:.3},{:.3}", p.x, p.y).unwrap(),
                    Segment::Line(p) => write!(out, "L{:.3},{:.3}", p.x, p.y).unwrap(),
                    Segment::Cubic(a, b, c) => write!(
                        out,
                        "C{:.3},{:.3} {:.3},{:.3} {:.3},{:.3}",
                        a.x, a.y, b.x, b.y, c.x, c.y
                    )
                    .unwrap(),
                    Segment::Close => out.push('Z'),
                }
            }
            out.push_str("\"/>");
        }
        out.push_str("</g></svg>\n");
        Ok(out.into_bytes())
    }
}
