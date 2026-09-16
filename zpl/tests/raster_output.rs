use std::ops::Range;
use zpl::output::{
    raster::{rasterize, rasterize_into, Raster, RasterOutput},
    Draw, OutputError, Paint, Path, Scene,
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
