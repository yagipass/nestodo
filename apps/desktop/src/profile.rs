use std::io::Cursor;

use image::imageops::FilterType;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};
use serde::Deserialize;

use crate::error::Error;

pub const AVATAR_SIZE: u32 = 256;
pub const MAX_AVATAR_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub email: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
}

fn unreadable(error: impl std::fmt::Display) -> Error {
    Error::Invalid(format!("the image could not be read: {error}"))
}

fn upright(bytes: &[u8]) -> Result<DynamicImage, Error> {
    let mut decoder = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(unreadable)?
        .into_decoder()
        .map_err(unreadable)?;
    let orientation = decoder.orientation().map_err(unreadable)?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(unreadable)?;
    image.apply_orientation(orientation);
    Ok(image)
}

pub fn avatar_png(bytes: &[u8]) -> Result<Vec<u8>, Error> {
    let square = upright(bytes)?.resize_to_fill(AVATAR_SIZE, AVATAR_SIZE, FilterType::Lanczos3);
    let square = if square.color().has_alpha() {
        DynamicImage::ImageRgba8(square.to_rgba8())
    } else {
        DynamicImage::ImageRgb8(square.to_rgb8())
    };
    let mut png = Cursor::new(Vec::new());
    square
        .write_to(&mut png, ImageFormat::Png)
        .map_err(unreadable)?;
    let png = png.into_inner();
    if png.len() > MAX_AVATAR_BYTES {
        return Err(Error::Invalid("the image is too large to send".into()));
    }
    Ok(png)
}

#[cfg(test)]
mod tests {
    use image::{Rgb, RgbImage, Rgba, RgbaImage};

    use super::*;

    const RED: Rgb<u8> = Rgb([220, 30, 30]);
    const BLUE: Rgb<u8> = Rgb([30, 30, 220]);

    fn encoded(image: &DynamicImage, format: ImageFormat) -> Vec<u8> {
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, format).unwrap();
        bytes.into_inner()
    }

    fn left_red_right_blue(width: u32, height: u32) -> RgbImage {
        RgbImage::from_fn(width, height, |x, _| if x < width / 2 { RED } else { BLUE })
    }

    fn decoded(png: &[u8]) -> DynamicImage {
        image::load_from_memory_with_format(png, ImageFormat::Png).unwrap()
    }

    fn is_red(image: &DynamicImage, x: u32, y: u32) -> bool {
        let [r, _, b] = image.to_rgb8().get_pixel(x, y).0;
        r > 150 && b < 100
    }

    fn is_blue(image: &DynamicImage, x: u32, y: u32) -> bool {
        let [r, _, b] = image.to_rgb8().get_pixel(x, y).0;
        b > 150 && r < 100
    }

    fn with_exif_orientation(jpeg: &[u8], orientation: u8) -> Vec<u8> {
        let mut exif = b"Exif\0\0MM\0\x2a\0\0\0\x08\0\x01\x01\x12\0\x03\0\0\0\x01\0".to_vec();
        exif.extend([orientation, 0, 0, 0, 0, 0, 0]);
        let length = u16::try_from(exif.len() + 2).unwrap().to_be_bytes();
        let mut tagged = jpeg[..2].to_vec();
        tagged.extend([0xff, 0xe1, length[0], length[1]]);
        tagged.extend(exif);
        tagged.extend(&jpeg[2..]);
        tagged
    }

    #[test]
    fn a_wide_photo_becomes_a_small_centered_square_the_server_accepts() {
        let wide = encoded(
            &DynamicImage::ImageRgb8(left_red_right_blue(3000, 1000)),
            ImageFormat::Jpeg,
        );

        let png = avatar_png(&wide).unwrap();

        let square = decoded(&png);
        assert_eq!(
            (square.width(), square.height()),
            (AVATAR_SIZE, AVATAR_SIZE)
        );
        assert!(png.len() <= MAX_AVATAR_BYTES);
        assert!(is_red(&square, 10, 128));
        assert!(is_blue(&square, 245, 128));
    }

    #[test]
    fn even_a_noisy_full_color_image_with_transparency_stays_under_the_servers_size_limit() {
        let mut seed: u32 = 1;
        let noisy = RgbaImage::from_fn(900, 900, |_, _| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let [r, g, b, a] = seed.to_le_bytes();
            Rgba([r, g, b, a])
        });

        let png = avatar_png(&encoded(&DynamicImage::ImageRgba8(noisy), ImageFormat::Png)).unwrap();

        assert!(png.len() <= MAX_AVATAR_BYTES, "{} bytes", png.len());
    }

    #[test]
    fn a_photo_taken_with_the_camera_turned_sideways_is_sent_upright() {
        let jpeg = encoded(
            &DynamicImage::ImageRgb8(left_red_right_blue(400, 200)),
            ImageFormat::Jpeg,
        );

        let square = decoded(&avatar_png(&with_exif_orientation(&jpeg, 6)).unwrap());

        assert!(is_red(&square, 128, 10));
        assert!(is_blue(&square, 128, 245));
    }

    #[test]
    fn a_file_that_is_not_an_image_is_refused_before_anything_is_sent() {
        assert!(matches!(
            avatar_png(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>"),
            Err(Error::Invalid(_))
        ));
    }
}
