/* TODO:
    - Overhaul bounding box location:
        - Current code can find a false corner when the topmost corner is a white pixel, so
          we need to find all four corners.
        - Should be doable by finding the apparent corners and then:
            - Finding if there's one pair of approximately matching adjacent slopes, two pairs, or all four match
            - If one pair match, adjust the corner that is on neither of them
            - If two pairs match, find which pair has more black pixels outside of it and adjust their shared corner
            - If all four match, the code is likely already correctly bounded
            - On consideration, should also be possible to look at the angle between adjacent edges.
                - Specifically, there are two possibilities:
                    - If one corner is empty, the corner closest to a 90 degree angle is the locator corner which
                      has two neighbours, and so we know it and its neighbours are filled corners, and can adjust
                      the final corner to match.
                    - If no corner is empty, finding the naive corners has already found the true corners.
                  the locator corner with two neighbours), and we can assume its adjacen
        - Finding the true corners also allows determining the orientation *and* the pixel pitch by walking in from
          the corners to find the black–white–longer black–white–black pattern of a locator corner
        - Determining the pixel pitch without reference to code version (as the current implementation does) also
          allows us to determine code version based on how far apart the paired locator corners are, so we can
          either implement multiple version support, or at least warn that we were expecting a V1 code but it
          does not appear to be one.
    - Implement reading for V1 codes.
 */

use imageproc::geometric_transformations as geom;
use imageproc::image;
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

fn shrinkwrap_bounding_box(img: &image::GrayImage, x1: u32, y1: u32, x2: u32, y2: u32) -> (u32, u32, u32, u32) {
    let (mut x1, mut y1, mut x2, mut y2) = (x1, y1, x2, y2);

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

fn decode_qr(input: image::DynamicImage) -> Result<String, Box<dyn std::error::Error>> {
    let input = input.into_luma8();

    // Find the top left corner of the bounding box of the QR code.
    let (w, h) = (input.width(), input.height());
    let (x1, y1, x2, y2) = shrinkwrap_bounding_box(&input, 0, 0, input.width() - 1, input.height() - 1);
    
    // Translate the image so the bounding box is in the bottom right.
    // Correct the coordinates afterwards.
    let offset_x = (w - 1) - x2;
    let offset_y = (h - 1) - y2;

    let input = geom::translate(
        &input,
        (offset_x as i32, offset_y as i32)
    );

    let x1 = x1 + offset_x;
    let y1 = y1 + offset_y;

    // Find the leftmost pixel on the top row, and topmost pixel on the left column.
    let (x2, _, _, _) = shrinkwrap_bounding_box(&input, x1, y1, w - 1, y1);
    let (_, y2, _, _) = shrinkwrap_bounding_box(&input, x1, y1, x1, h - 1);

    // Determine the angle needed to rotate the code to upright.
    let tri_w = (x2 - x1) as f32;
    let tri_h = (y2 - y1) as f32;

    let correction_angle = (tri_h / tri_w).atan();

    // Rotate the image based on the calculated angle.
    let corrected = geom::rotate(
        &input,
        (x2 as f32, y1 as f32),
        correction_angle,
        geom::Interpolation::Bilinear,
        image::Luma::from([255u8])
    );

    corrected.save_with_format("debug_corrected.png", image::ImageFormat::Png)?;

    let (x1, y1, x2, y2) = shrinkwrap_bounding_box(&corrected, 0, 0, corrected.width() - 1, corrected.height() - 1);
    let cropped = image::imageops::crop_imm(&corrected, x1, y1, x2 - x1, y2 - y1).to_image();
    cropped.save_with_format("debug_cropped.png", image::ImageFormat::Png)?;

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

    // rotate the QR code so that the ID lines are on the top and left
    // if they aren't already
    let rotated = if id_lower == ID_PATTERN && id_left == ID_PATTERN {
        image::imageops::rotate90(&cropped)
    } else if id_right == ID_PATTERN && id_lower == ID_PATTERN {
        image::imageops::rotate180(&cropped)
    } else if id_upper == ID_PATTERN && id_right == ID_PATTERN {
        image::imageops::rotate270(&cropped)
    } else {
        cropped
    };

    rotated.save_with_format("debug_rotated.png", image::ImageFormat::Png)?;

    Ok("dummy".into())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();

    let client = reqwest::blocking::Client::new();
    let token = std::env::var("HACKATTIC_TOKEN").unwrap_or("DUMMY".into());
    let dry_run = std::env::var("DRY_RUN").unwrap_or("".into()) != "";

    let qr: image::DynamicImage = if dry_run {
        image::ImageReader::open("test_input.png")?
            .decode()?
    } else {
        let problem_url = format!("https://hackattic.com/challenges/reading_qr/problem?access_token={}", token);
        let resp = client.get(&problem_url).send()?;
        let resp: HackatticQRResponse = serde_json::from_str(&resp.text()?)?;
        let resp = client.get(resp.image_url).send()?;
        image::ImageReader::new(std::io::Cursor::new(resp.bytes()?))
            .with_guessed_format()?
            .decode()?
    };
    
    qr.save_with_format("debug_input.png", image::ImageFormat::Png)?;
    
    let solution = decode_qr(qr)?;

    if dry_run {
        println!("Got solution: {}", solution);
    } else {
        let solve_url = format!("https://hackattic.com/challenges/reading_qr/solve?access_token={}", token);
        let resp = client.post(&solve_url)
            .body(serde_json::to_string(&HackatticQRRequest { code: solution })?)
            .send()?;
        println!("Server response:");
        println!("{}", resp.text()?);
    }

    Ok(())
}
