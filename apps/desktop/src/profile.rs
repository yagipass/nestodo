use std::io::Cursor;

use image::codecs::jpeg::JpegEncoder;
use image::imageops::{self, FilterType};
use image::metadata::Orientation;
use image::{
    DynamicImage, ImageDecoder, ImageError, ImageFormat, ImageReader, Limits, Rgb, RgbImage,
};
use serde::Deserialize;

use crate::error::Error;

pub const AVATAR_SIZE: u32 = 256;
pub const MAX_AVATAR_BYTES: usize = 512 * 1024;
pub const ICON_SIZE: u32 = 128;
pub const MAX_ICON_BYTES: usize = 128 * 1024;
pub const MAX_SOURCE_SIDE: u32 = 16_384;
pub const MAX_SOURCE_BYTES: u64 = 256 * 1024 * 1024;
pub const TOO_LARGE: &str = "the image is too large to read";
pub const COMMENT_IMAGE_EDGE: u32 = 2048;
pub const MAX_COMMENT_IMAGE_BYTES: usize = 2 * 1024 * 1024;
const COMMENT_QUALITIES: [u8; 3] = [85, 70, 50];
const MIN_COMMENT_IMAGE_EDGE: u32 = 256;
const TOO_LARGE_TO_SEND: &str = "the image is too large to send";

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

fn not_decoded(error: ImageError) -> Error {
    match error {
        ImageError::Limits(_) => Error::Invalid(TOO_LARGE.into()),
        error => unreadable(error),
    }
}

fn decode_limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SOURCE_SIDE);
    limits.max_image_height = Some(MAX_SOURCE_SIDE);
    limits.max_alloc = Some(MAX_SOURCE_BYTES);
    limits
}

fn decode(bytes: &[u8]) -> Result<(DynamicImage, Orientation), Error> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(unreadable)?;
    reader.limits(decode_limits());
    let mut decoder = reader.into_decoder().map_err(not_decoded)?;
    if decoder.total_bytes() > MAX_SOURCE_BYTES {
        return Err(Error::Invalid(TOO_LARGE.into()));
    }
    let orientation = decoder.orientation().map_err(not_decoded)?;
    let image = DynamicImage::from_decoder(decoder).map_err(not_decoded)?;
    if image.width() == 0 || image.height() == 0 {
        return Err(unreadable("the image is empty"));
    }
    Ok((image, orientation))
}

fn upright_square(bytes: &[u8], target: u32) -> Result<DynamicImage, Error> {
    let (image, orientation) = decode(bytes)?;
    let side = image.width().min(image.height());
    let middle = imageops::crop_imm(
        &image,
        (image.width() - side) / 2,
        (image.height() - side) / 2,
        side,
        side,
    );
    let square = imageops::resize(&*middle, target, target, FilterType::Lanczos3);
    let mut square = if image.color().has_alpha() {
        DynamicImage::ImageRgba8(square)
    } else {
        DynamicImage::ImageRgb8(DynamicImage::ImageRgba8(square).to_rgb8())
    };
    square.apply_orientation(orientation);
    Ok(square)
}

pub fn avatar_png(bytes: &[u8]) -> Result<Vec<u8>, Error> {
    square_png(bytes, AVATAR_SIZE, MAX_AVATAR_BYTES)
}

pub fn icon_png(bytes: &[u8]) -> Result<Vec<u8>, Error> {
    square_png(bytes, ICON_SIZE, MAX_ICON_BYTES)
}

fn square_png(bytes: &[u8], target: u32, max_bytes: usize) -> Result<Vec<u8>, Error> {
    let square = upright_square(bytes, target)?;
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
    if png.len() > max_bytes {
        return Err(Error::Invalid(TOO_LARGE_TO_SEND.into()));
    }
    Ok(png)
}

#[must_use]
pub fn fit_within(width: u32, height: u32, edge: u32) -> (u32, u32) {
    let longer = width.max(height);
    if longer <= edge {
        return (width.max(1), height.max(1));
    }
    let scaled = |side: u32| {
        let side = (u64::from(side) * u64::from(edge) + u64::from(longer) / 2) / u64::from(longer);
        u32::try_from(side).unwrap_or(edge).max(1)
    };
    (scaled(width), scaled(height))
}

fn on_white(image: &DynamicImage) -> RgbImage {
    if !image.color().has_alpha() {
        return image.to_rgb8();
    }
    let rgba = image.to_rgba8();
    RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let [r, g, b, a] = rgba.get_pixel(x, y).0;
        let blend = |channel: u8| {
            let mixed =
                (u16::from(channel) * u16::from(a) + 255 * (255 - u16::from(a)) + 127) / 255;
            u8::try_from(mixed).unwrap_or(u8::MAX)
        };
        Rgb([blend(r), blend(g), blend(b)])
    })
}

pub fn comment_jpeg(bytes: &[u8]) -> Result<Vec<u8>, Error> {
    let (image, orientation) = decode(bytes)?;
    let (width, height) = fit_within(image.width(), image.height(), COMMENT_IMAGE_EDGE);
    let image = if (width, height) == (image.width(), image.height()) {
        image
    } else {
        image.thumbnail_exact(width, height)
    };
    let mut upright = DynamicImage::ImageRgb8(on_white(&image));
    upright.apply_orientation(orientation);
    for quality in COMMENT_QUALITIES {
        if let Some(jpeg) = jpeg_within_limit(&upright, quality)? {
            return Ok(jpeg);
        }
    }
    let lowest = COMMENT_QUALITIES[COMMENT_QUALITIES.len() - 1];
    while upright.width().max(upright.height()) > MIN_COMMENT_IMAGE_EDGE {
        upright = upright.thumbnail_exact(
            (upright.width() * 3 / 4).max(1),
            (upright.height() * 3 / 4).max(1),
        );
        if let Some(jpeg) = jpeg_within_limit(&upright, lowest)? {
            return Ok(jpeg);
        }
    }
    Err(Error::Invalid(TOO_LARGE_TO_SEND.into()))
}

fn jpeg_within_limit(image: &DynamicImage, quality: u8) -> Result<Option<Vec<u8>>, Error> {
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, quality)
        .encode_image(image)
        .map_err(unreadable)?;
    Ok((jpeg.len() <= MAX_COMMENT_IMAGE_BYTES).then_some(jpeg))
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
    fn an_opaque_photo_is_sent_without_an_alpha_channel_so_it_stays_small() {
        let photo = encoded(
            &DynamicImage::ImageRgb8(left_red_right_blue(600, 400)),
            ImageFormat::Jpeg,
        );

        let square = decoded(&avatar_png(&photo).unwrap());

        assert_eq!(square.color(), image::ColorType::Rgb8);
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
    fn an_icon_image_is_the_middle_of_the_picture_at_128_pixels_and_even_noise_stays_under_the_servers_128_kb_limit()
     {
        let wide = encoded(
            &DynamicImage::ImageRgb8(left_red_right_blue(900, 300)),
            ImageFormat::Jpeg,
        );
        let square = decoded(&icon_png(&wide).unwrap());
        assert_eq!((square.width(), square.height()), (ICON_SIZE, ICON_SIZE));
        assert!(is_red(&square, 8, 64));
        assert!(is_blue(&square, 119, 64));

        let mut seed: u32 = 7;
        let noisy = RgbaImage::from_fn(700, 700, |_, _| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let [r, g, b, a] = seed.to_le_bytes();
            Rgba([r, g, b, a])
        });
        let png = icon_png(&encoded(&DynamicImage::ImageRgba8(noisy), ImageFormat::Png)).unwrap();
        assert!(png.len() <= MAX_ICON_BYTES, "{} bytes", png.len());
        assert_eq!(decoded(&png).width(), ICON_SIZE);
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
        assert!(matches!(
            comment_jpeg(b"not a picture"),
            Err(Error::Invalid(_))
        ));
    }

    fn decoded_jpeg(jpeg: &[u8]) -> DynamicImage {
        image::load_from_memory_with_format(jpeg, ImageFormat::Jpeg).unwrap()
    }

    #[test]
    fn a_large_photo_for_a_comment_is_shrunk_until_its_longer_side_is_2048_pixels_keeping_all_of_it_in_shape()
     {
        let wide = encoded(
            &DynamicImage::ImageRgb8(left_red_right_blue(3000, 1000)),
            ImageFormat::Jpeg,
        );

        let sent = decoded_jpeg(&comment_jpeg(&wide).unwrap());

        assert_eq!((sent.width(), sent.height()), (COMMENT_IMAGE_EDGE, 683));
        assert!(is_red(&sent, 10, 340));
        assert!(is_blue(&sent, 2037, 340));
    }

    #[test]
    fn a_small_screenshot_for_a_comment_keeps_its_own_size_since_enlarging_only_blurs_it() {
        let screenshot = encoded(
            &DynamicImage::ImageRgb8(left_red_right_blue(400, 300)),
            ImageFormat::Png,
        );

        let sent = decoded_jpeg(&comment_jpeg(&screenshot).unwrap());

        assert_eq!((sent.width(), sent.height()), (400, 300));
    }

    #[test]
    fn a_transparent_part_of_a_comment_picture_turns_white_rather_than_black_since_the_picture_is_sent_without_transparency()
     {
        let clear = encoded(
            &DynamicImage::ImageRgba8(RgbaImage::from_pixel(40, 40, Rgba([0, 0, 0, 0]))),
            ImageFormat::Png,
        );

        let sent = decoded_jpeg(&comment_jpeg(&clear).unwrap()).to_rgb8();

        let [r, g, b] = sent.get_pixel(20, 20).0;
        assert!(r > 240 && g > 240 && b > 240, "{r} {g} {b}");
    }

    #[test]
    fn a_comment_photo_taken_with_the_camera_turned_sideways_is_sent_upright() {
        let jpeg = encoded(
            &DynamicImage::ImageRgb8(left_red_right_blue(400, 200)),
            ImageFormat::Jpeg,
        );

        let sent = decoded_jpeg(&comment_jpeg(&with_exif_orientation(&jpeg, 6)).unwrap());

        assert_eq!((sent.width(), sent.height()), (200, 400));
        assert!(is_red(&sent, 100, 10));
        assert!(is_blue(&sent, 100, 390));
    }

    #[test]
    fn even_a_noisy_comment_picture_at_full_size_is_sent_under_the_servers_2_mb_limit_instead_of_being_refused()
     {
        let mut seed: u32 = 3;
        let noisy = RgbImage::from_fn(COMMENT_IMAGE_EDGE, COMMENT_IMAGE_EDGE, |_, _| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let [r, g, b, _] = seed.to_le_bytes();
            Rgb([r, g, b])
        });

        let jpeg =
            comment_jpeg(&encoded(&DynamicImage::ImageRgb8(noisy), ImageFormat::Png)).unwrap();

        assert!(
            jpeg.len() <= MAX_COMMENT_IMAGE_BYTES,
            "{} bytes",
            jpeg.len()
        );
    }

    #[test]
    fn a_picture_is_fitted_inside_the_edge_by_its_longer_side_and_never_loses_a_side() {
        assert_eq!(fit_within(4032, 3024, 2048), (2048, 1536));
        assert_eq!(fit_within(1170, 2532, 2048), (946, 2048));
        assert_eq!(fit_within(800, 600, 2048), (800, 600));
        assert_eq!(fit_within(10_000, 1, 2048), (2048, 1));
    }

    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = 0xffff_ffff_u32;
        for &byte in bytes {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ 0xedb8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }

    fn chunk(kind: &[u8], data: &[u8]) -> Vec<u8> {
        let mut chunk = u32::try_from(data.len()).unwrap().to_be_bytes().to_vec();
        chunk.extend(kind);
        chunk.extend(data);
        chunk.extend(crc32(&[kind, data].concat()).to_be_bytes());
        chunk
    }

    fn png_claiming(width: u32, height: u32) -> Vec<u8> {
        let mut header = width.to_be_bytes().to_vec();
        header.extend(height.to_be_bytes());
        header.extend([8, 2, 0, 0, 0]);
        let empty_zlib = [
            0x78, 0x01, 0x01, 0x00, 0x00, 0xff, 0xff, 0x00, 0x00, 0x00, 0x01,
        ];
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        png.extend(chunk(b"IHDR", &header));
        png.extend(chunk(b"IDAT", &empty_zlib));
        png.extend(chunk(b"IEND", &[]));
        png
    }

    fn refused_as_too_large(bytes: &[u8]) -> bool {
        matches!(avatar_png(bytes), Err(Error::Invalid(message)) if message == TOO_LARGE)
    }

    #[test]
    fn a_tiny_file_claiming_a_gigantic_picture_is_refused_before_decoding_so_the_app_cannot_run_out_of_memory()
     {
        assert!(refused_as_too_large(&png_claiming(11_000, 11_000)));
        assert!(refused_as_too_large(&png_claiming(20_000, 10)));
        assert!(refused_as_too_large(&png_claiming(10, 20_000)));
    }

    #[test]
    fn a_long_thin_strip_is_cut_to_its_middle_before_scaling_so_a_tiny_file_cannot_need_gigabytes_while_resizing()
     {
        let strip = encoded(
            &DynamicImage::ImageRgb8(RgbImage::from_pixel(MAX_SOURCE_SIDE, 1, RED)),
            ImageFormat::Png,
        );

        let square = decoded(&avatar_png(&strip).unwrap());

        assert_eq!(
            (square.width(), square.height()),
            (AVATAR_SIZE, AVATAR_SIZE)
        );
        assert!(is_red(&square, 128, 128));
    }

    #[test]
    fn a_48_megapixel_phone_photo_is_not_refused_for_its_size() {
        let photo = png_claiming(8064, 6048);

        assert!(!refused_as_too_large(&photo));
    }
}
