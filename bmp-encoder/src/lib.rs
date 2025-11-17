use std::{
    io::Write,
    marker::PhantomData,
    ops::{BitAndAssign, BitOrAssign},
};

use byteorder::{LittleEndian, WriteBytesExt};

pub trait WriteData: Sized {
    fn write_into<W: Write>(&self, writer: &mut W) -> Result<(), std::io::Error>;
}

impl WriteData for Header {
    fn write_into<W: Write>(&self, writer: &mut W) -> Result<(), std::io::Error> {
        writer.write_all(&self.inner)?;
        Ok(())
    }
}

#[derive(Clone)]
struct Header {
    inner: [u8; 2],
}

impl Default for Header {
    fn default() -> Self {
        Self {
            inner: [66, 77], // the chars 'BM' in ascii
        }
    }
}

#[derive(Clone)]
struct BmpHeader {
    file_size: u32,
    creator_1: u16,
    creator_2: u16,
    pixel_offset: u32,
}

impl BmpHeader {
    fn new(data_size: u32) -> Self {
        BmpHeader {
            file_size: HEADER_SIZE + data_size,
            creator_1: 0,
            creator_2: 0,
            pixel_offset: HEADER_SIZE,
        }
    }
}

impl WriteData for BmpHeader {
    fn write_into<W: Write>(&self, writer: &mut W) -> Result<(), std::io::Error> {
        writer.write_u32::<LittleEndian>(self.file_size)?;
        writer.write_u16::<LittleEndian>(self.creator_1)?;
        writer.write_u16::<LittleEndian>(self.creator_2)?;
        writer.write_u32::<LittleEndian>(self.pixel_offset)?;
        Ok(())
    }
}

#[derive(Clone)]
struct BmpDibHeader {
    header_size: u32,
    width: u32,
    height: u32,
    num_planes: u16,
    bits_per_pixel: u16,
    compress_type: u32,
    data_size: DataSize,
    hres: i32,
    vres: i32,
    num_colors: u32,
    num_imp_colors: u32,
}

impl BmpDibHeader {
    fn new(bits_per_pixel: u16, width: u32, height: u32) -> Self {
        let data = DataSize::new(bits_per_pixel, width, height);
        let num_colors = if bits_per_pixel == 1 { 2 } else { 0 };
        Self {
            header_size: 40,
            width,
            height,
            num_planes: 1,
            bits_per_pixel,
            compress_type: 0,
            data_size: data,
            hres: 1000,
            vres: 1000,
            num_colors,
            num_imp_colors: num_colors,
        }
    }
}

impl WriteData for BmpDibHeader {
    fn write_into<W: Write>(&self, writer: &mut W) -> Result<(), std::io::Error> {
        writer.write_u32::<LittleEndian>(self.header_size)?;
        writer.write_u32::<LittleEndian>(self.width)?;
        writer.write_u32::<LittleEndian>(self.height)?;
        writer.write_u16::<LittleEndian>(self.num_planes)?;
        writer.write_u16::<LittleEndian>(self.bits_per_pixel)?;
        writer.write_u32::<LittleEndian>(self.compress_type)?;
        writer.write_u32::<LittleEndian>(self.data_size.0)?;
        writer.write_i32::<LittleEndian>(self.hres)?;
        writer.write_i32::<LittleEndian>(self.vres)?;
        writer.write_u32::<LittleEndian>(self.num_colors)?;
        writer.write_u32::<LittleEndian>(self.num_imp_colors)?;
        Ok(())
    }
}

// Color palette for 1-bit BMP (2 colors: black and white)
// Each color is 4 bytes: Blue, Green, Red, Reserved (BGRA format)
#[derive(Clone)]
struct ColorPalette {
    colors: [u8; 8], // 2 colors * 4 bytes each
}

impl Default for ColorPalette {
    fn default() -> Self {
        Self {
            // Color 0: Black (0, 0, 0, 0)
            // Color 1: White (255, 255, 255, 0)
            // This is the correct order - reversed palette triggers image_reverse in firmware
            colors: [0, 0, 0, 0, 255, 255, 255, 0],
        }
    }
}

impl WriteData for ColorPalette {
    fn write_into<W: Write>(&self, writer: &mut W) -> Result<(), std::io::Error> {
        writer.write_all(&self.colors)?;
        Ok(())
    }
}

const COLOR_PALETTE_SIZE: u32 = 8; // 2 colors * 4 bytes each for 1-bit BMP

const HEADER_SIZE: u32 = size_of::<Header>() as u32
    + size_of::<BmpHeader>() as u32
    + size_of::<BmpDibHeader>() as u32
    + COLOR_PALETTE_SIZE;

/// the size of the image data in bytes
#[repr(transparent)]
#[derive(Clone)]
struct DataSize(u32);

impl DataSize {
    /// return the size in bytes of the data (non header) part of the image
    fn new(bitspp: u16, width: u32, height: u32) -> DataSize {
        // find row size in bytes, round up to 4 bytes (padding)
        let row_size = ((bitspp as f32 * width as f32 + 31.0) / 32.0).floor() as u32 * 4;
        DataSize(height * row_size)
    }
}

#[derive(Clone)]
pub struct Image<P> {
    header: Header,
    bmp_header: BmpHeader,
    dib_header: BmpDibHeader,
    color_palette: ColorPalette,
    data: ImageData<P>,
}

impl<P> Image<P>
where
    P: Pixel,
{
    pub fn new(width: u32, height: u32) -> Self {
        let data = ImageData::new(width, height);
        Self {
            header: Header::default(),
            bmp_header: BmpHeader::new(data.final_size()),
            dib_header: BmpDibHeader::new(P::BITS_PER_PIXEL, width, height),
            color_palette: ColorPalette::default(),
            data,
        }
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.data.width, self.data.height)
    }

    pub fn image_bytes(&self) -> Result<Vec<u8>, std::io::Error> {
        let s = self.bmp_header.file_size;
        let mut out = Vec::with_capacity(s as usize);
        self.write_into(&mut out)?;
        Ok(out)
    }
}

#[derive(Clone)]
struct ImageData<P> {
    width: u32,
    height: u32,
    data: Vec<u8>,
    phantom: PhantomData<P>,
    padding_size: u32,
    bytes_per_row: u32,
}

pub trait Pixel {
    type Data;
    const BITS_PER_PIXEL: u16;
    fn index_mut(buf: &mut [u8], idx: usize, val: Self::Data) -> Option<()>;
}

#[derive(Clone)]
pub struct U1;

struct DivAndMod {
    dividend: u32,
    modulus: u32,
}

impl DivAndMod {
    fn new(v: u32) -> Self {
        let byte_idx = v / 8;
        let bit_idx = v % 8;
        DivAndMod {
            dividend: byte_idx,
            modulus: bit_idx,
        }
    }
}

impl Pixel for U1 {
    type Data = bool;
    const BITS_PER_PIXEL: u16 = 1;

    fn index_mut(buf: &mut [u8], idx: usize, val: Self::Data) -> Option<()> {
        let DivAndMod {
            dividend: byte_idx,
            modulus: bit_idx,
        } = DivAndMod::new(idx as u32);
        let byte = buf.get_mut(byte_idx as usize)?;
        match val {
            true => {
                let mask = 1 << 7 >> bit_idx;
                byte.bitor_assign(mask);
            }
            false => {
                let mask = !(1 << 7 >> bit_idx);
                byte.bitand_assign(mask);
            }
        };
        Some(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Coord {
    pub x: u32,
    pub y: u32,
}

impl<P> Image<P>
where
    P: Pixel<Data = bool>,
{
    pub fn draw_pixels(&mut self, iter: impl IntoIterator<Item = (Coord, bool)>) {
        let width = self.data.width;
        let get_idx = move |c: Coord| c.y * width + c.x;
        iter.into_iter()
            .map(|(c, v)| (get_idx(c), v))
            .for_each(|(idx, v)| {
                P::index_mut(&mut self.data.data, idx as usize, v);
            });
    }
}

impl<P> ImageData<P>
where
    P: Pixel,
{
    fn new(width: u32, height: u32) -> Self {
        let DivAndMod {
            dividend: mut bytes_per_row,
            modulus,
        } = DivAndMod::new(width * P::BITS_PER_PIXEL as u32);
        if modulus > 0 {
            bytes_per_row += 1;
        }
        let capacity = bytes_per_row as usize * height as usize;
        let mut data = Vec::with_capacity(capacity);
        data.resize(capacity, Default::default());
        // Calculate padding to align rows to 4-byte boundaries
        let padding_size = (4 - (bytes_per_row % 4)) % 4;
        Self {
            width,
            height,
            data,
            phantom: PhantomData,
            padding_size,
            bytes_per_row,
        }
    }

    fn final_size(&self) -> u32 {
        (self.bytes_per_row + self.padding_size) * self.height
    }
}

impl<P> WriteData for ImageData<P>
where
    P: Pixel,
{
    fn write_into<W: Write>(&self, writer: &mut W) -> Result<(), std::io::Error> {
        // BMP images are stored bottom-to-top, so iterate in reverse
        for y in (0..self.height).rev() {
            let start = y * self.bytes_per_row;
            let end = start + self.bytes_per_row;
            writer.write_all(&self.data[start as usize..end as usize])?;
            writer.write_all(&[0; 4][0..self.padding_size as usize])?;
        }
        Ok(())
    }
}

impl<P> WriteData for Image<P>
where
    P: Pixel,
{
    fn write_into<W: Write>(&self, writer: &mut W) -> Result<(), std::io::Error> {
        self.header.write_into(writer)?;
        self.bmp_header.write_into(writer)?;
        self.dib_header.write_into(writer)?;
        self.color_palette.write_into(writer)?;
        self.data.write_into(writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bmp_header_size() {
        assert_eq!(size_of::<BmpHeader>(), 12);
    }

    #[test]
    fn dib_header_size() {
        assert_eq!(size_of::<BmpDibHeader>(), 40);
    }

    #[test]
    fn header_size_is_62() {
        // 2 (BM) + 12 (BmpHeader) + 40 (DibHeader) + 8 (ColorPalette) = 62
        assert_eq!(HEADER_SIZE, 62);
    }

    #[test]
    fn it_writes_2_bytes() {
        let mut buf = Vec::new();
        (Header::default().write_into(&mut buf).unwrap());
        assert_eq!(buf.len(), 2)
    }

    #[test]
    fn it_writes_bits_1() {
        let mut buf = vec![255, 255];
        U1::index_mut(&mut buf, 9, false);
        assert_eq!(buf, vec![255, 191]);
    }

    #[test]
    fn it_writes_bits_2() {
        let mut buf = vec![255, 255];
        U1::index_mut(&mut buf, 8, false);
        assert_eq!(buf, vec![255, 127]);
    }

    #[test]
    fn it_writes_bits_3() {
        let mut buf = vec![255, 255];
        U1::index_mut(&mut buf, 0, false);
        assert_eq!(buf, vec![127, 255]);
    }

    #[test]
    fn temp() {
        assert_eq!(255 >> 1, 127);
    }

    #[test]
    fn it_writes_bits_4() {
        let mut buf = vec![255, 255];
        U1::index_mut(&mut buf, 1, false);
        assert_eq!(buf, vec![191, 255]);
    }

    #[test]
    fn it_writes_bits_5() {
        let mut buf = vec![255, 255];
        U1::index_mut(&mut buf, 7, false);
        assert_eq!(buf, vec![254, 255]);
    }

    #[test]
    fn it_should_not_be_homogeneous() {
        let mut i = Image::<U1>::new(800, 480);
        i.draw_pixels((0..800).flat_map(|x| (0..480).map(move |y| (Coord { x, y }, x % 2 == 0))));
        let bytes = i.image_bytes().unwrap();
        assert!(!bytes[54..].iter().all(|b| b == &0));
        assert!(!bytes[54..].iter().all(|b| b == &u8::MAX));
    }
}
