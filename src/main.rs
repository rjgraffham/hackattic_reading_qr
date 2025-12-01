use imageproc::geometric_transformations::{rotate, Interpolation};
use imageproc::image::{imageops, DynamicImage, GrayImage, ImageFormat, ImageReader, Luma};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct HackatticQRResponse {
    image_url: String,
}

#[derive(Serialize, Deserialize)]
struct HackatticQRRequest {
    code: String,
}

const ID_PATTERN: [bool; 21] = [
    true, true, true, true, true, true, true,
    false, true, false, true, false, true, false,
    true, true, true, true, true, true, true
];

fn find_bounding_box(img: &GrayImage) -> (u32, u32, u32, u32) {
    let mut x1 = 0;
    let mut y1 = 0;
    let mut x2 = img.width() - 1;
    let mut y2 = img.height() - 1;

    while y1 < y2 {
        let mut found_edge = false;
        for x in 0..=x2 {
            if img.get_pixel(x, y1).0[0] < 16 {
                found_edge = true;
                break;
            }
        }

        if found_edge {
            break;
        } else {
            y1 += 1;
        }
    };

    while x1 < x2 {
        let mut found_edge = false;
        for y in y1..=y2 {
            if img.get_pixel(x1, y).0[0] < 16 {
                found_edge = true;
                break;
            }
        }

        if found_edge {
            break;
        } else {
            x1 += 1;
        }
    };

    while y2 > y1 {
        let mut found_edge = false;
        for x in x1..=x2 {
            if img.get_pixel(x, y2).0[0] < 16 {
                found_edge = true;
                break;
            }
        }

        if found_edge {
            break;
        } else {
            y2 -= 1;
        }
    };

    while x2 > x1 {
        let mut found_edge = false;
        for y in y1..=y2 {
            if img.get_pixel(x2, y).0[0] < 16 {
                found_edge = true;
                break;
            }
        }

        if found_edge {
            break;
        } else {
            x2 -= 1;
        }
    };

    (x1, y1, x2, y2)
}

fn decode_qr(input: DynamicImage) -> Result<(), Box<dyn std::error::Error>> {
    let input = input.into_luma8();

    // Find the top left corner of the bounding box of the QR code.
    let (w, h) = (input.width(), input.height());
    let (x1, y1, _, _) = find_bounding_box(&input);

    // Find the leftmost pixel on the top row, and topmost pixel on the left column.
    let mut x2 = x1;
    let mut y2 = y1;

    while x2 < w {
        if input.get_pixel(x2, y1).0[0] < 16 {
            break;
        }

        x2 += 1;
    }

    while y2 < h {
        if input.get_pixel(x1, y2).0[0] < 16 {
            break;
        }
        
        y2 += 1;
    }

    // Determine the angle needed to rotate the code to upright.
    let tri_w = (x2 - x1) as f32;
    let tri_h = (y2 - y1) as f32;

    let correction_angle = (tri_h / tri_w).atan();

    // Rotate the image based on the calculated angle.
    let corrected = rotate(
        &input,
        (x2 as f32, y1 as f32),
        correction_angle,
        Interpolation::Bilinear,
        Luma::from([255u8])
    );

    corrected.save_with_format("debug_corrected.png", ImageFormat::Png)?;

    let (x1, y1, x2, y2) = find_bounding_box(&corrected);
    let cropped = imageops::crop_imm(&corrected, x1, y1, x2 - x1, y2 - y1).to_image();
    cropped.save_with_format("debug_cropped.png", ImageFormat::Png)?;

    // From here, we assume that we have a V1 QR code (21×21), determine the pitch,
    // and check which orientation it is.
    let pixel_pitch = (cropped.width() as f32 / 21.0).round() as u32;

    let mut id_upper = [false; 21];
    let mut id_left = [false; 21];
    let mut id_right = [false; 21];
    let mut id_lower = [false; 21];

    for x in 0..21 {
        id_upper[x] = cropped.get_pixel(
            (x as u32 * pixel_pitch) + (pixel_pitch / 2),
            (6 * pixel_pitch) + (pixel_pitch / 2)
        ).0[0] < 127;
        id_lower[x] = cropped.get_pixel(
            (x as u32 * pixel_pitch) + (pixel_pitch / 2),
            (14 * pixel_pitch) + (pixel_pitch / 2)
        ).0[0] < 127;
    }

    for y in 0..21 {
        id_left[y] = cropped.get_pixel(
            (6 * pixel_pitch) + (pixel_pitch / 2),
            (y as u32 * pixel_pitch) + (pixel_pitch / 2)
        ).0[0] < 127;
        id_right[y] = cropped.get_pixel(
            (14 * pixel_pitch) + (pixel_pitch / 2),
            (y as u32 * pixel_pitch) + (pixel_pitch / 2)
        ).0[0] < 127;
    }

    if id_upper == ID_PATTERN { println!("ID line found on upper.") }
    if id_left == ID_PATTERN { println!("ID line found on left.") }
    if id_right == ID_PATTERN { println!("ID line found on right.") }
    if id_lower == ID_PATTERN { println!("ID line found on lower.") }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    /* For testing, we will use the same local image every time. For the final
       version we will instead get a dynamic image from an HTTP request. To
       ease this conversion, we will delegate the actual logic to a function
       that takes an Image and operates on it without knowing where it came
       from.
     */
    let qr = ImageReader::open("E:\\Downloads\\7a0ad415.2a22.47e1.94e1.png")?;
    let qr = qr.decode()?;
    decode_qr(qr)?;

    // let client = reqwest::blocking::Client::new();

    // let resp = client.get("https://hackattic.com/challenges/reading_qr/problem?access_token=DUMMY").send()?;
    // let resp: HackatticQRResponse = serde_json::from_str(&resp.text()?)?;
    // let resp = client.get(resp.image_url).send()?;
    // let qr = ImageReader::new(std::io::Cursor::new(resp.bytes()?))
    //     .with_guessed_format()?
    //     .decode()?;

    // qr.save_with_format("debug_downloaded.png", ImageFormat::Png)?;

    // decode_qr(qr)?;

    // let resp = client.post("https://hackattic.com/challenges/reading_qr/solve?access_token=DUMMY")
    //     .body(serde_json::to_string(&HackatticQRRequest { code: "dummy".into() })?)
    //     .send()?;

    // Ok(println!("{}", resp.text()?))

    Ok(())
}
