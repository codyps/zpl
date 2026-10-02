use std::ops::Range;
use zpl::output::{
    raster::{rasterize, rasterize_into, Raster, RasterOutput},
    Adapter, Draw, OutputError, Paint, Path, Png, Scene,
};

// A packed destination exercises compositing without a grayscale pixel buffer.
#[derive(Default)]
struct Packed {
    width: u32,
    height: u32,
    bits: Vec<u8>,
}
impl RasterOutput for Packed {
    fn reset(&mut self, width: u32, height: u32) -> Result<(), OutputError> {
        self.width = width;
        self.height = height;
        self.bits = vec![0; (width as usize * height as usize).div_ceil(8)];
        Ok(())
    }

    fn paint_span(&mut self, y: u32, x: Range<u32>, paint: Paint) -> Result<(), OutputError> {
        assert!(y < self.height && x.start < x.end && x.end <= self.width);
        for x in x {
            let i = (y * self.width + x) as usize;
            let bit = 1 << (i % 8);
            match paint {
                Paint::Black => self.bits[i / 8] |= bit,
                Paint::White => self.bits[i / 8] &= !bit,
                Paint::Invert => self.bits[i / 8] ^= bit,
            }
        }
        Ok(())
    }
}

fn layered_scene() -> Scene {
    let mut scene = Scene::new(4, 3, 203).unwrap();
    for (x, y, w, h, paint) in [
        (-1., 0., 4., 2., Paint::Black),
        (1., 0., 1., 1., Paint::White),
        (2., 0., 3., 3., Paint::Invert),
        (10., 10., 1., 1., Paint::Black),
    ] {
        let mut path = Path::default();
        path.rect(x, y, w, h);
        scene.draws.push(Draw { path, paint });
    }
    scene
}

#[test]
fn packed_output_preserves_clipping_and_paint_order() {
    let scene = layered_scene();
    let mut output = Packed::default();
    let sink: &mut dyn RasterOutput = &mut output;
    rasterize_into(&scene, sink).unwrap();
    let expected = [0, 255, 255, 0, 0, 0, 255, 0, 255, 255, 0, 0];
    for (i, pixel) in expected.iter().enumerate() {
        assert_eq!(output.bits[i / 8] & (1 << (i % 8)) != 0, *pixel == 0);
    }
    assert_eq!(rasterize(&scene).unwrap().pixels, expected);
}

#[test]
fn raster_output_resets_existing_storage_and_dimensions() {
    let mut output = rasterize(&layered_scene()).unwrap();
    let capacity = output.pixels.capacity();
    rasterize_into(&Scene::new(2, 2, 203).unwrap(), &mut output).unwrap();
    assert_eq!(
        output,
        Raster {
            width: 2,
            height: 2,
            pixels: vec![255; 4]
        }
    );
    assert_eq!(output.pixels.capacity(), capacity);
    rasterize_into(&layered_scene(), &mut output).unwrap();
    assert_eq!(output, rasterize(&layered_scene()).unwrap());
}

struct FailingOutput {
    resets: usize,
    spans: usize,
    fail_reset: bool,
}
impl RasterOutput for FailingOutput {
    fn reset(&mut self, _: u32, _: u32) -> Result<(), OutputError> {
        self.resets += 1;
        if self.fail_reset {
            Err(OutputError("reset failed"))
        } else {
            Ok(())
        }
    }
    fn paint_span(&mut self, _: u32, _: Range<u32>, _: Paint) -> Result<(), OutputError> {
        self.spans += 1;
        Err(OutputError("span failed"))
    }
}

#[test]
fn destination_errors_stop_rasterization() {
    for fail_reset in [true, false] {
        let mut output = FailingOutput {
            resets: 0,
            spans: 0,
            fail_reset,
        };
        assert_eq!(
            rasterize_into(&layered_scene(), &mut output),
            Err(OutputError(if fail_reset {
                "reset failed"
            } else {
                "span failed"
            }))
        );
        assert_eq!(output.resets, 1);
        assert_eq!(output.spans, usize::from(!fail_reset));
    }
}

#[test]
fn invalid_scene_does_not_touch_destination() {
    let mut scene = layered_scene();
    scene.width = 0;
    let mut output = FailingOutput {
        resets: 0,
        spans: 0,
        fail_reset: false,
    };
    assert!(rasterize_into(&scene, &mut output).is_err());
    assert_eq!((output.resets, output.spans), (0, 0));
}

#[test]
fn scan_budget_ignores_vertically_inactive_contours() {
    // 12,000 short disjoint contours exceed the old all-edges * height
    // estimate, but have only 24,000 actual vertical edge/row visits.
    let mut scene = Scene::new(120, 4000, 203).unwrap();
    let mut path = Path::default();
    for row in 0..2000 {
        for column in 0..6 {
            path.rect(f64::from(column * 20), f64::from(row * 2), 2., 1.);
        }
    }
    scene.draws.push(Draw {
        path,
        paint: Paint::Black,
    });
    let actual = rasterize(&scene).unwrap();
    for y in 0..4000usize {
        for x in 0..120usize {
            assert_eq!(actual.pixels[y * 120 + x] == 0, y % 2 == 0 && x % 20 < 2);
        }
    }
}

#[test]
fn scan_budget_still_limits_active_edge_work() {
    let mut scene = Scene::new(10, 10000, 203).unwrap();
    let mut path = Path::default();
    for _ in 0..5001 {
        path.rect(1., 0., 2., 10000.);
    }
    scene.draws.push(Draw {
        path,
        paint: Paint::Black,
    });
    assert_eq!(
        rasterize(&scene),
        Err(OutputError("raster scan budget exceeded"))
    );
}

#[test]
fn vertical_crossings_match_pixel_centers_across_events_and_paints() {
    // Pixel-center, half-open, even-odd contract: docs/local-renderer.md.
    // Fractional edges must enter/leave at the crossing row, not floor/ceil.
    let rectangles = [
        (-2.25, -0.5, 13.75, 5.5),
        (1.5, 0.5, 9.5, 3.5),
        (3.25, 3.25, 7.75, 6.75),
        (0.5, 6.5, 16.5, 8.5),
        (4., 8.1, 12., 8.4), // no pixel-center crossing
    ];
    let mut scene = Scene::new(17, 10, 203).unwrap();
    let mut expected = vec![255; 170];
    for (shift, paint) in [
        (0., Paint::Black),
        (0.25, Paint::White),
        (1., Paint::Invert),
    ] {
        let mut path = Path::default();
        for &(left, top, right, bottom) in &rectangles {
            path.rect(left + shift, top, right - left, bottom - top);
        }
        scene.draws.push(Draw { path, paint });
        for y in 0..10 {
            for x in 0..17 {
                let (cx, cy) = (x as f64 + 0.5, y as f64 + 0.5);
                let crossings = rectangles
                    .iter()
                    .filter(|&&(left, top, right, bottom)| {
                        left + shift <= cx && cx < right + shift && top <= cy && cy < bottom
                    })
                    .count();
                if crossings % 2 == 1 {
                    let p = &mut expected[y * 17 + x];
                    *p = match paint {
                        Paint::Black => 0,
                        Paint::White => 255,
                        Paint::Invert => 255 - *p,
                    };
                }
            }
        }
    }
    assert_eq!(rasterize(&scene).unwrap().pixels, expected);
    let decoded = Raster::decode_png_with_threshold(&Png.encode(&scene).unwrap(), None).unwrap();
    assert_eq!(decoded.pixels, expected);
}

#[test]
fn packed_png_matches_grayscale_for_curves_clipping_and_inversion() {
    let mut scene = layered_scene();
    scene.width = 37;
    scene.height = 29;
    for paint in [Paint::Black, Paint::White, Paint::Invert] {
        let mut path = Path::default();
        path.ellipse(-3.25, 1.5, 38.5, 24.75);
        path.ellipse(4.25, 5.25, 17., 9.5);
        scene.draws.push(Draw { path, paint });
        assert_eq!(
            Raster::decode_png_with_threshold(&Png.encode(&scene).unwrap(), None).unwrap(),
            rasterize(&scene).unwrap(),
        );
    }
}
