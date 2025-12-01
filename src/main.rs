use imageproc::geometric_transformations::{rotate, Interpolation};
use imageproc::image::{imageops, DynamicImage, GrayImage, ImageFormat, ImageReader, Luma};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct HackatticQRResponse {
    image_url: String,
}

struct HackatticQRRequest {
    code: String,
}

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
    let (w, h) = (input.width(), input.height());
    println!("We have a grayscale image of dimensions {} x {}", w, h);

    // Find the top left corner of the bounding box of the QR code.
    let (x1, y1, _, _) = find_bounding_box(&input);

    println!("Found QR code with bounding box beginning at [{}, {}]", x1, y1);

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

    let tri_w = (x2 - x1) as f32;
    let tri_h = (y2 - y1) as f32;

    println!("Found a triangle {} wide and {} tall.", tri_w, tri_h);
    
    let correction_angle = (tri_h / tri_w).atan();

    let corrected = rotate(
        &input,
        (x2 as f32, y1 as f32),
        correction_angle,
        Interpolation::Bilinear,
        Luma::from([255u8])
    );

    let (x1, y1, x2, y2) = find_bounding_box(&corrected);

    println!("After correction, new bounding box is [{}, {}] -> [{}, {}]", x1, y1, x2, y2);

    corrected.save_with_format("debug_corrected.png", ImageFormat::Png)?;

    let cropped = imageops::crop_imm(&corrected, x1, y1, x2 - x1, y2 - y1).to_image();

    cropped.save_with_format("debug_cropped.png", ImageFormat::Png)?;

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    /* For testing, we will use the same local image every time. For the final
       version we will instead get a dynamic image from an HTTP request. To
       ease this conversion, we will delegate the actual logic to a function
       that takes an Image and operates on it without knowing where it came
       from.
     */
    // let qr = ImageReader::open("E:\\Downloads\\7a0ad415.2a22.47e1.94e1.png")?;
    // let qr = qr.decode()?;
    // decode_qr(qr)?;

    let resp = reqwest::blocking::get("https://hackattic.com/challenges/reading_qr/problem?access_token=DUMMY")?;
    let resp: HackatticQRResponse = serde_json::from_str(&resp.text()?)?;
    let resp = reqwest::blocking::get(resp.image_url)?;
    let qr = ImageReader::new(std::io::Cursor::new(resp.bytes()?))
        .with_guessed_format()?
        .decode()?;

    qr.save_with_format("debug_downloaded.png", ImageFormat::Png)?;

    decode_qr(qr)
}
