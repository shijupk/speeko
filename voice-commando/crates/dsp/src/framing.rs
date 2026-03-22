use std::f32::consts::PI;

/// Precompute Hamming window coefficients for a given frame length.
pub fn hamming_window(length: usize) -> Vec<f32> {
    (0..length)
        .map(|n| 0.54 - 0.46 * (2.0 * PI * n as f32 / (length as f32 - 1.0)).cos())
        .collect()
}

/// Split audio into overlapping frames.
///
/// Returns a Vec of frames, each of length `frame_length`.
/// Frames are separated by `frame_step` samples.
/// The last partial frame is discarded.
pub fn split_frames(samples: &[f32], frame_length: usize, frame_step: usize) -> Vec<Vec<f32>> {
    if samples.len() < frame_length {
        return vec![];
    }

    let num_frames = (samples.len() - frame_length) / frame_step + 1;
    let mut frames = Vec::with_capacity(num_frames);

    for i in 0..num_frames {
        let start = i * frame_step;
        let end = start + frame_length;
        frames.push(samples[start..end].to_vec());
    }

    frames
}

/// Apply a window function (e.g., Hamming) to a frame in-place.
pub fn apply_window(frame: &mut [f32], window: &[f32]) {
    debug_assert_eq!(frame.len(), window.len(), "Frame and window must be the same length");
    for (s, &w) in frame.iter_mut().zip(window.iter()) {
        *s *= w;
    }
}

/// Split audio into windowed frames (framing + Hamming window in one step).
pub fn windowed_frames(
    samples: &[f32],
    frame_length: usize,
    frame_step: usize,
) -> Vec<Vec<f32>> {
    let window = hamming_window(frame_length);
    let mut frames = split_frames(samples, frame_length, frame_step);
    for frame in frames.iter_mut() {
        apply_window(frame, &window);
    }
    frames
}
