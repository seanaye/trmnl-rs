use bmp_encoder::{Image, U1};
use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::{Dimensions, DrawTarget},
};
use std::{rc::Rc, sync::Mutex};

#[derive(Clone)]
pub struct BmpWrapper(Rc<Mutex<Image<U1>>>);

impl BmpWrapper {
    pub fn new(width: u32, height: u32) -> BmpWrapper {
        BmpWrapper(Rc::new(Mutex::new(Image::new(width, height))))
    }

    pub fn data(&self) -> Result<Vec<u8>, std::io::Error> {
        let guard = self.0.lock().unwrap();
        guard.image_bytes()
    }
}

impl Dimensions for BmpWrapper {
    fn bounding_box(&self) -> embedded_graphics::primitives::Rectangle {
        let (width, height) = self.0.lock().unwrap().dimensions();
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
        let iter = pixels.into_iter().map(|p| {
            (
                bmp_encoder::Coord {
                    x: p.0.x as u32,
                    y: p.0.y as u32,
                },
                p.1.is_on(),
            )
        });
        self.0.lock().unwrap().draw_pixels(iter);
        Ok(())
    }
}
