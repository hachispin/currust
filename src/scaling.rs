//! Contains scaling algorithms.

/// Nearest-neighbour scaling algorithm for RGBA8.
///
/// This is center-aligned and used for *upscaling*.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
pub(crate) fn scale_nearest(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
    let mut dst = vec![0u8; (dst_w * dst_h * 4) as usize];

    let scale_x = src_w as f32 / dst_w as f32;
    let scale_y = src_h as f32 / dst_h as f32;

    // lowkenuinely copied from chatgpt so i have
    // no clue what this does. it works though
    for y in 0..dst_h {
        for x in 0..dst_w {
            let src_x = ((x as f32 + 0.5) * scale_x - 0.5)
                .round()
                .clamp(0.0, (src_w - 1) as f32) as u32;

            let src_y = ((y as f32 + 0.5) * scale_y - 0.5)
                .round()
                .clamp(0.0, (src_h - 1) as f32) as u32;

            let src_idx = ((src_y * src_w + src_x) * 4) as usize;
            let dst_idx = ((y * dst_w + x) * 4) as usize;

            dst[dst_idx..dst_idx + 4].copy_from_slice(&src[src_idx..src_idx + 4]);
        }
    }

    dst
}

/// Box sampling/averaging algorithm for RGBA8.
///
/// This is used for *downscaling*.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
pub(crate) fn scale_box_average(
    src: &[u8],
    src_w: u32,
    src_h: u32,
    dst_w: u32,
    dst_h: u32,
) -> Vec<u8> {
    let mut dst = vec![0u8; (dst_w * dst_h * 4) as usize];

    let scale_x = src_w as f32 / dst_w as f32;
    let scale_y = src_h as f32 / dst_h as f32;

    // same thing with this. just copied so i'm
    // clueles on what this actually does
    for y in 0..dst_h {
        for x in 0..dst_w {
            // find the source rectangle this dst pixel covers
            let x0 = (x as f32 * scale_x).floor() as u32;
            let y0 = (y as f32 * scale_y).floor() as u32;
            let x1 = ((x + 1) as f32 * scale_x).ceil().min(src_w as f32) as u32;
            let y1 = ((y + 1) as f32 * scale_y).ceil().min(src_h as f32) as u32;

            let mut r_sum = 0u32;
            let mut g_sum = 0u32;
            let mut b_sum = 0u32;
            let mut a_sum = 0u32;
            let mut count = 0u32;

            for sy in y0..y1 {
                for sx in x0..x1 {
                    let idx = ((sy * src_w + sx) * 4) as usize;
                    r_sum += u32::from(src[idx]);
                    g_sum += u32::from(src[idx + 1]);
                    b_sum += u32::from(src[idx + 2]);
                    a_sum += u32::from(src[idx + 3]);
                    count += 1;
                }
            }

            let dst_idx = ((y * dst_w + x) * 4) as usize;
            dst[dst_idx] = (r_sum / count) as u8;
            dst[dst_idx + 1] = (g_sum / count) as u8;
            dst[dst_idx + 2] = (b_sum / count) as u8;
            dst[dst_idx + 3] = (a_sum / count) as u8;
        }
    }

    dst
}
