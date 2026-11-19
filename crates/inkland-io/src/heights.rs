//! Height images: the PNG file of an `image` template.

use inkland_core::config::{Config, Raster, Template};
use inkland_core::error::{Error, ErrorCode};

/// The largest image side, in pixels.
pub const MAX_SIDE: u32 = 8192;

/// Decode a PNG image of heights into a raster. Black is height 0 and
/// white is height 100. A colour pixel gives its brightness. The alpha
/// channel has no effect. The first row of the raster is the top row of
/// the image.
pub fn raster_from_png(bytes: &[u8]) -> Result<Raster, Error> {
    let bad = |m: String| Error::new(ErrorCode::TemplateInvalid, "/world/template/image", m);
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|e| bad(format!("the image is not a PNG file: {e}")))?;
    let (cols, rows) = (reader.info().width, reader.info().height);
    if cols < 2 || rows < 2 || cols > MAX_SIDE || rows > MAX_SIDE {
        return Err(bad(format!(
            "the image is {cols} x {rows} pixels; each side must be 2 to {MAX_SIDE}"
        )));
    }
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| bad("the image is too large".into()))?;
    let mut buf = vec![0u8; size];
    let frame = reader
        .next_frame(&mut buf)
        .map_err(|e| bad(format!("cannot decode the image: {e}")))?;
    let channels = match frame.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        // EXPAND turns an indexed image into RGB or RGBA.
        png::ColorType::Indexed => {
            return Err(bad("the image has a colour type that is not known".into()));
        }
    };
    let line = frame.line_size;
    let mut values = Vec::with_capacity(cols as usize * rows as usize);
    for row in buf[..frame.buffer_size()].chunks(line).take(rows as usize) {
        for px in row.chunks(channels).take(cols as usize) {
            let gray = if channels >= 3 {
                // The brightness (Rec. 601), in whole numbers.
                (299 * u32::from(px[0]) + 587 * u32::from(px[1]) + 114 * u32::from(px[2]) + 500)
                    / 1000
            } else {
                u32::from(px[0])
            };
            values.push(((gray * 100 + 127) / 255) as u8);
        }
    }
    Ok(Raster { cols, rows, values })
}

/// Read the image of an `image` template. `read` gets the path of the image
/// as the config gives it and returns the bytes of the file. A config with
/// another template does not change.
pub fn load_template_image(
    config: &mut Config,
    read: impl FnOnce(&str) -> std::io::Result<Vec<u8>>,
) -> Result<(), Error> {
    let Template::Image { image, heights } = &mut config.world.template else {
        return Ok(());
    };
    let bytes = read(image).map_err(|e| {
        Error::new(
            ErrorCode::IoRead,
            "/world/template/image",
            format!("cannot read the image of the template: {e}"),
        )
        .with("file", image.clone())
    })?;
    *heights = Some(raster_from_png(&bytes)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(w: u32, h: u32, color: png::ColorType, bit16: bool, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut e = png::Encoder::new(&mut out, w, h);
            e.set_color(color);
            e.set_depth(if bit16 {
                png::BitDepth::Sixteen
            } else {
                png::BitDepth::Eight
            });
            let mut writer = e.write_header().unwrap();
            writer.write_image_data(data).unwrap();
        }
        out
    }

    #[test]
    fn a_gray_image_gives_heights_from_0_to_100() {
        let png = encode(2, 2, png::ColorType::Grayscale, false, &[0, 255, 51, 128]);
        let r = raster_from_png(&png).unwrap();
        assert_eq!((r.cols, r.rows), (2, 2));
        assert_eq!(r.values, vec![0, 100, 20, 50]);
    }

    #[test]
    fn a_colour_image_gives_the_brightness() {
        #[rustfmt::skip]
        let data = [
            255, 255, 255, 255,   0, 0, 0, 255,
            255, 0, 0, 0,         0, 0, 255, 255,
        ];
        let png = encode(2, 2, png::ColorType::Rgba, false, &data);
        let r = raster_from_png(&png).unwrap();
        // Red: 0.299 x 255 = 76, which is height 30. Blue: 29, height 11.
        assert_eq!(r.values, vec![100, 0, 30, 11]);
    }

    #[test]
    fn a_16_bit_image_uses_the_high_byte() {
        let png = encode(
            2,
            2,
            png::ColorType::Grayscale,
            true,
            &[255, 255, 0, 0, 128, 0, 0, 255],
        );
        assert_eq!(raster_from_png(&png).unwrap().values, vec![100, 0, 50, 0]);
    }

    #[test]
    fn a_file_that_is_not_a_png_or_too_small_is_refused() {
        let e = raster_from_png(b"not a png").unwrap_err();
        assert_eq!(e.code, ErrorCode::TemplateInvalid);
        let one = encode(1, 3, png::ColorType::Grayscale, false, &[0, 0, 0]);
        assert_eq!(
            raster_from_png(&one).unwrap_err().code,
            ErrorCode::TemplateInvalid
        );
    }

    #[test]
    fn load_template_image_sets_the_heights_and_reports_a_missing_file() {
        let mut c: Config = serde_json::from_value(serde_json::json!({
            "version": 1, "seed": 1, "frame": { "width": 100, "height": 100 },
            "world": { "template": { "image": "h.png" } }
        }))
        .unwrap();
        let png = encode(2, 2, png::ColorType::Grayscale, false, &[0, 255, 0, 255]);
        let mut asked = String::new();
        load_template_image(&mut c, |p| {
            asked = p.to_owned();
            Ok(png.clone())
        })
        .unwrap();
        assert_eq!(asked, "h.png");
        assert_eq!(
            c.world.template.raster().unwrap().values,
            vec![0, 100, 0, 100]
        );

        let e = load_template_image(&mut c, |_| {
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "no file"))
        })
        .unwrap_err();
        assert_eq!(e.code, ErrorCode::IoRead);
    }
}
