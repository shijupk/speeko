use speeko_common::config::SpeekConfig;
use speeko_common::types::Template;
use speeko_dsp::preprocess;
use speeko_features::mfcc::MfccExtractor;
use speeko_recognizer::matcher::TemplateMatcher;
use speeko_vad::energy_vad;

/// Generate a synthetic "word" as a sine wave at a given frequency.
fn synthetic_word(freq: f32, duration_secs: f32, sample_rate: u32) -> Vec<f32> {
    let num_samples = (sample_rate as f32 * duration_secs) as usize;
    let silence_before = vec![0.001; (sample_rate as f32 * 0.3) as usize];
    let silence_after = vec![0.001; (sample_rate as f32 * 0.3) as usize];

    let mut samples = silence_before;
    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        samples.push(0.5 * (2.0 * std::f32::consts::PI * freq * t).sin());
    }
    samples.extend(silence_after);
    samples
}

#[test]
fn test_full_pipeline_synthetic() {
    let config = SpeekConfig::default();
    let mut mfcc_extractor =
        MfccExtractor::new(config.audio.sample_rate, &config.dsp, &config.mfcc);

    // Create two synthetic "words" with different frequencies.
    let word_a_freq = 300.0;
    let word_b_freq = 1000.0;

    // Create templates.
    let mut templates = Vec::new();
    for i in 0..3 {
        let mut samples = synthetic_word(word_a_freq + i as f32 * 5.0, 0.5, config.audio.sample_rate);
        preprocess::preprocess(&mut samples, config.dsp.pre_emphasis);

        let trimmed = energy_vad::trim_silence(
            &samples,
            config.audio.sample_rate,
            config.frame_length_samples(),
            config.frame_step_samples(),
            &config.vad,
        )
        .unwrap_or(samples);

        let mfcc = mfcc_extractor.extract(&trimmed);
        templates.push(Template {
            word: "alpha".to_string(),
            sample_index: i,
            mfcc,
        });
    }

    for i in 0..3 {
        let mut samples = synthetic_word(word_b_freq + i as f32 * 5.0, 0.5, config.audio.sample_rate);
        preprocess::preprocess(&mut samples, config.dsp.pre_emphasis);

        let trimmed = energy_vad::trim_silence(
            &samples,
            config.audio.sample_rate,
            config.frame_length_samples(),
            config.frame_step_samples(),
            &config.vad,
        )
        .unwrap_or(samples);

        let mfcc = mfcc_extractor.extract(&trimmed);
        templates.push(Template {
            word: "beta".to_string(),
            sample_index: i,
            mfcc,
        });
    }

    let matcher = TemplateMatcher::new(
        config.recognizer.sakoe_chiba_width,
        config.recognizer.confidence_threshold,
        config.recognizer.max_distance,
    );

    // Test: a query similar to "alpha" should be recognized as "alpha".
    let mut query_a = synthetic_word(word_a_freq, 0.5, config.audio.sample_rate);
    preprocess::preprocess(&mut query_a, config.dsp.pre_emphasis);
    let trimmed_a = energy_vad::trim_silence(
        &query_a,
        config.audio.sample_rate,
        config.frame_length_samples(),
        config.frame_step_samples(),
        &config.vad,
    )
    .unwrap_or(query_a);
    let mfcc_a = mfcc_extractor.extract(&trimmed_a);
    let result_a = matcher.recognize(&mfcc_a, &templates);

    assert_eq!(
        result_a.word,
        Some("alpha".to_string()),
        "Expected 'alpha', got {:?} (conf={:.2}, dist={:.2})",
        result_a.word,
        result_a.confidence,
        result_a.best_distance
    );

    // Test: a query similar to "beta" should be recognized as "beta".
    let mut query_b = synthetic_word(word_b_freq, 0.5, config.audio.sample_rate);
    preprocess::preprocess(&mut query_b, config.dsp.pre_emphasis);
    let trimmed_b = energy_vad::trim_silence(
        &query_b,
        config.audio.sample_rate,
        config.frame_length_samples(),
        config.frame_step_samples(),
        &config.vad,
    )
    .unwrap_or(query_b);
    let mfcc_b = mfcc_extractor.extract(&trimmed_b);
    let result_b = matcher.recognize(&mfcc_b, &templates);

    assert_eq!(
        result_b.word,
        Some("beta".to_string()),
        "Expected 'beta', got {:?} (conf={:.2}, dist={:.2})",
        result_b.word,
        result_b.confidence,
        result_b.best_distance
    );
}

#[test]
fn test_mfcc_determinism() {
    let config = SpeekConfig::default();
    let mut extractor = MfccExtractor::new(config.audio.sample_rate, &config.dsp, &config.mfcc);

    let samples: Vec<f32> = (0..16000)
        .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16000.0).sin() * 0.5)
        .collect();

    let mfcc1 = extractor.extract(&samples);
    let mfcc2 = extractor.extract(&samples);

    assert_eq!(mfcc1.len(), mfcc2.len());
    for (f1, f2) in mfcc1.iter().zip(mfcc2.iter()) {
        for (a, b) in f1.iter().zip(f2.iter()) {
            assert!(
                (a - b).abs() < 1e-6,
                "MFCC not deterministic: {} vs {}",
                a,
                b
            );
        }
    }
}
