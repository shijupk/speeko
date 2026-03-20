use std::time::Instant;

use speeko_common::config::SpeekConfig;
use speeko_common::types::Template;
use speeko_dsp::preprocess;
use speeko_features::mfcc::MfccExtractor;
use speeko_recognizer::matcher::TemplateMatcher;

fn synthetic_word(freq: f32, duration_secs: f32, sample_rate: u32) -> Vec<f32> {
    let num_samples = (sample_rate as f32 * duration_secs) as usize;
    (0..num_samples)
        .map(|i| 0.5 * (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate as f32).sin())
        .collect()
}

fn main() {
    let config = SpeekConfig::default();
    let mut extractor = MfccExtractor::new(config.audio.sample_rate, &config.dsp, &config.mfcc);

    // Create 10 words x 5 templates each.
    let freqs: Vec<f32> = (0..10).map(|i| 200.0 + i as f32 * 100.0).collect();
    let mut templates = Vec::new();

    for (word_idx, &freq) in freqs.iter().enumerate() {
        for sample in 0..5 {
            let mut samples = synthetic_word(freq + sample as f32 * 2.0, 0.8, config.audio.sample_rate);
            preprocess::preprocess(&mut samples, config.dsp.pre_emphasis);
            let mfcc = extractor.extract(&samples);
            templates.push(Template {
                word: format!("word_{}", word_idx),
                sample_index: sample,
                mfcc,
            });
        }
    }

    let matcher = TemplateMatcher::new(
        config.recognizer.sakoe_chiba_width,
        config.recognizer.confidence_threshold,
        config.recognizer.max_distance,
    );

    // Benchmark: MFCC extraction.
    let query_samples = synthetic_word(300.0, 0.8, config.audio.sample_rate);
    let iterations = 100;

    let start = Instant::now();
    for _ in 0..iterations {
        let mut s = query_samples.clone();
        preprocess::preprocess(&mut s, config.dsp.pre_emphasis);
        let _ = extractor.extract(&s);
    }
    let mfcc_elapsed = start.elapsed();
    println!(
        "MFCC extraction: {:.2}ms avg ({} iterations)",
        mfcc_elapsed.as_millis() as f64 / iterations as f64,
        iterations
    );

    // Benchmark: DTW matching (10 words x 5 templates = 50 comparisons).
    let mut query = query_samples.clone();
    preprocess::preprocess(&mut query, config.dsp.pre_emphasis);
    let query_mfcc = extractor.extract(&query);

    let start = Instant::now();
    for _ in 0..iterations {
        let _ = matcher.recognize(&query_mfcc, &templates);
    }
    let dtw_elapsed = start.elapsed();
    println!(
        "DTW matching (50 templates): {:.2}ms avg ({} iterations)",
        dtw_elapsed.as_millis() as f64 / iterations as f64,
        iterations
    );

    // Benchmark: full pipeline.
    let start = Instant::now();
    for _ in 0..iterations {
        let mut s = query_samples.clone();
        preprocess::preprocess(&mut s, config.dsp.pre_emphasis);
        let mfcc = extractor.extract(&s);
        let _ = matcher.recognize(&mfcc, &templates);
    }
    let total_elapsed = start.elapsed();
    println!(
        "Full pipeline (preprocess + MFCC + DTW): {:.2}ms avg ({} iterations)",
        total_elapsed.as_millis() as f64 / iterations as f64,
        iterations
    );
}
