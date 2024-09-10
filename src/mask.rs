//! WebSocket frame masking implementation.

use rand::Rng;

/// Masks a WebSocket frame.
///
/// This function applies a random mask to the frame data and prepends the mask to the data.
pub fn mask_frame(data: &mut [u8]) {
    let mut rng = rand::thread_rng();
    let mask: [u8; 4] = rng.gen();

    for (i, byte) in data.iter_mut().enumerate() {
        *byte ^= mask[i % 4];
    }

    // Insert the mask at the beginning of the data
    data.rotate_right(4);
    data[..4].copy_from_slice(&mask);
}
