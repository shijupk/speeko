use ringbuf::traits::Split;
use ringbuf::HeapRb;

/// Create a lock-free SPSC ring buffer for audio samples.
///
/// At 16 kHz mono and buffer_secs=2.0, holds 32,000 samples (~128 KB).
/// Overflow policy: drop-new (producer drops incoming if full).
pub fn create_ring_buffer(
    sample_rate: u32,
    buffer_secs: f32,
) -> (ringbuf::HeapProd<f32>, ringbuf::HeapCons<f32>) {
    let capacity = (sample_rate as f32 * buffer_secs) as usize;
    let rb = HeapRb::<f32>::new(capacity);
    rb.split()
}
