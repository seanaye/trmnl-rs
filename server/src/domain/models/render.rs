use bmp_encoder::{Image, U1};
use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::{Dimensions, DrawTarget},
};
use std::{rc::Rc, sync::Mutex};

#[derive(Clone)]
pub struct BmpWrapper {
    image: Rc<Mutex<Image<U1>>>,
    scale: usize,
}

impl BmpWrapper {
    pub fn new_with_scale(width: u32, height: u32, scale: usize) -> BmpWrapper {
        BmpWrapper {
            image: Rc::new(Mutex::new(Image::new(width, height))),
            scale,
        }
    }

    pub fn data(&self) -> Result<Vec<u8>, std::io::Error> {
        let guard = self.image.lock().unwrap();
        guard.image_bytes()
    }
}

impl Dimensions for BmpWrapper {
    fn bounding_box(&self) -> embedded_graphics::primitives::Rectangle {
        let (width, height) = self.image.lock().unwrap().dimensions();
        let width = width / self.scale as u32;
        let height = height / self.scale as u32;
        embedded_graphics::primitives::Rectangle {
            top_left: Default::default(),
            size: embedded_graphics::prelude::Size { width, height },
        }
    }
}

impl DrawTarget for BmpWrapper {
    type Color = BinaryColor;

    type Error = std::io::Error;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
    {
        let iter = pixels.into_iter().flat_map(|p| {
            let is_on = !p.1.is_on();
            let scale = self.scale;
            (0..scale).flat_map(move |x| {
                (0..scale).map(move |y| {
                    (
                        bmp_encoder::Coord {
                            x: (p.0.x as u32 * scale as u32) + x as u32,
                            y: (p.0.y as u32 * scale as u32) + y as u32,
                        },
                        is_on,
                    )
                })
            })
        });
        self.image.lock().unwrap().draw_pixels(iter);
        Ok(())
    }
}
