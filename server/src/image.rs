//! An example of generating julia fractals.

use std::{
    io::{BufWriter, Cursor},
    sync::Arc,
};

use axum::{
    extract::Query,
    http::{StatusCode, header},
    response::IntoResponse,
};
use redb::Database;
use serde::Deserialize;

use crate::extractor::{api_key::ApiKeyExtractor, device_id::DeviceDimensions};

fn create_fractal(
    imgx: u32,
    imgy: u32,
    scale: f32,
) -> image::ImageBuffer<image::Luma<u8>, Vec<u8>> {
    let scalex = scale / imgx as f32;
    // let scaley = scale / imgy as f32;

    // Create a new ImgBuf with width: imgx and height: imgy
    let mut imgbuf = image::ImageBuffer::new(imgx, imgy);

    for x in 0..imgx {
        for y in 0..imgy {
            let cx = y as f32 * scalex - 1.5;
            let cy = x as f32 * scalex - 1.5;

            let c = num_complex::Complex::new(-0.4, 0.6);
            let mut z = num_complex::Complex::new(cx, cy);

            let mut i = 0;
            while i < 255 && z.norm() <= 2.0 {
                z = z * z + c;
                i += 1;
            }

            let pixel = imgbuf.get_pixel_mut(x, y);
            *pixel = image::Luma([i as u8]);
        }
    }

    imgbuf
}

#[derive(Deserialize)]
pub struct Params {
    h: u32,
    w: u32,
}

#[axum::debug_handler(state = Arc<Database>)]
pub async fn image_handler(Query(params): Query<Params>) -> Result<impl IntoResponse, StatusCode> {
    let buf = create_fractal(params.w, params.h, 2.0);
    let mut output = Cursor::new(Vec::new());
    let Ok(()) = buf.write_to(&mut output, image::ImageFormat::Png) else {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };
    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "image/png")],
        output.into_inner(),
    ))
}
