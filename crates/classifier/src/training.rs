use anyhow::{bail, Result};
use burn::backend::Autodiff;
use burn::backend::NdArray;
use burn::optim::{AdamConfig, GradientsParams, Optimizer};
use burn::prelude::*;
use burn::tensor::activation::log_softmax;

use crate::dataset::{self, LabeledSample, VocabMap};
use crate::model::{KeywordCnn, KeywordCnnConfig};

/// Backend types for training (NdArray with autodiff).
pub type TrainBackend = Autodiff<NdArray<f32>>;
pub type InferBackend = NdArray<f32>;

/// Train a CNN classifier from labeled MFCC samples.
///
/// Returns the path to the saved model directory.
pub fn train_cnn(
    train_samples: &[LabeledSample],
    val_samples: &[LabeledSample],
    vocab: &VocabMap,
    max_frames: usize,
    epochs: usize,
    batch_size: usize,
    learning_rate: f64,
    patience: usize,
    model_dir: &std::path::Path,
    num_augments: usize,
) -> Result<()> {
    if train_samples.is_empty() {
        bail!("No training samples");
    }
    if vocab.num_classes() < 2 {
        bail!("Need at least 2 classes to train a classifier");
    }

    let device = Default::default();
    let num_coeffs = train_samples[0].mfcc[0].len();

    let config = KeywordCnnConfig::new(vocab.num_classes())
        .with_input_channels(num_coeffs);
    let mut model: KeywordCnn<TrainBackend> = config.init(&device);

    let optim_config = AdamConfig::new();
    let mut optim = optim_config.init::<TrainBackend, KeywordCnn<TrainBackend>>();

    let mut best_val_acc = 0.0f32;
    let mut epochs_without_improvement = 0usize;

    println!("Training CNN classifier:");
    println!("  Classes: {}", vocab.num_classes());
    println!("  Training samples: {} (+ {}x augmentation)", train_samples.len(), num_augments);
    println!("  Validation samples: {}", val_samples.len());
    println!("  Max frames: {}", max_frames);
    println!("  Feature dims: {}", num_coeffs);
    println!("  Epochs: {}", epochs);
    println!("  Batch size: {}", batch_size);
    println!("  Learning rate: {}", learning_rate);
    println!();

    // Prepare validation data once (no augmentation).
    let (val_features, val_labels) = dataset::prepare_batch(val_samples, max_frames, false, 0);

    for epoch in 0..epochs {
        // Prepare training data with augmentation (re-augmented each epoch).
        let (train_features, train_labels) =
            dataset::prepare_batch(train_samples, max_frames, true, num_augments);

        let num_train = train_features.len();

        // Shuffle indices.
        let mut indices: Vec<usize> = (0..num_train).collect();
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        indices.shuffle(&mut rng);

        let mut epoch_loss = 0.0f32;
        let mut epoch_correct = 0usize;
        let mut epoch_total = 0usize;
        let num_batches = (num_train + batch_size - 1) / batch_size;

        for batch_idx in 0..num_batches {
            let start = batch_idx * batch_size;
            let end = (start + batch_size).min(num_train);
            let batch_indices: Vec<usize> = indices[start..end].to_vec();
            let bs = batch_indices.len();

            // Build input tensor [bs, C, T].
            let feature_len = num_coeffs * max_frames;
            let mut flat_input = Vec::with_capacity(bs * feature_len);
            let mut flat_labels = Vec::with_capacity(bs);
            for &idx in &batch_indices {
                flat_input.extend_from_slice(&train_features[idx]);
                flat_labels.push(train_labels[idx] as i64);
            }

            let input = Tensor::<TrainBackend, 1>::from_floats(
                flat_input.as_slice(),
                &device,
            )
            .reshape([bs, num_coeffs, max_frames]);

            let targets = Tensor::<TrainBackend, 1, Int>::from_ints(
                flat_labels.as_slice(),
                &device,
            );

            // Forward pass.
            let logits = model.forward(input);
            let log_probs = log_softmax(logits.clone(), 1);

            // Cross-entropy loss: -sum(one_hot * log_probs) / batch_size.
            let loss = cross_entropy_loss(log_probs.clone(), targets.clone(), vocab.num_classes(), &device);

            // Accuracy for this batch.
            let preds = logits.clone().argmax(1).squeeze::<1>();
            let correct = preds
                .equal(targets.clone())
                .int()
                .sum()
                .into_scalar();
            epoch_correct += correct as usize;
            epoch_total += bs;

            let loss_val: f32 = loss.clone().into_scalar().elem();
            epoch_loss += loss_val * bs as f32;

            // Backward + update.
            let grads = loss.backward();
            let grad_container = GradientsParams::from_grads(grads, &model);
            model = optim.step(learning_rate, model, grad_container);
        }

        let train_acc = epoch_correct as f32 / epoch_total.max(1) as f32;
        let avg_loss = epoch_loss / epoch_total.max(1) as f32;

        // Validation.
        let val_acc = evaluate_accuracy(&model, &val_features, &val_labels, num_coeffs, max_frames, &device);

        println!(
            "  Epoch {:3}/{}: loss={:.4}, train_acc={:.1}%, val_acc={:.1}%",
            epoch + 1,
            epochs,
            avg_loss,
            train_acc * 100.0,
            val_acc * 100.0,
        );

        // Early stopping check.
        if val_acc > best_val_acc {
            best_val_acc = val_acc;
            epochs_without_improvement = 0;

            // Save best model.
            save_model(&model, model_dir)?;
        } else {
            epochs_without_improvement += 1;
            if epochs_without_improvement >= patience {
                println!(
                    "  Early stopping at epoch {} (no improvement for {} epochs)",
                    epoch + 1,
                    patience
                );
                break;
            }
        }
    }

    println!();
    println!("Best validation accuracy: {:.1}%", best_val_acc * 100.0);
    println!("Model saved to {:?}", model_dir);

    Ok(())
}

/// Cross-entropy loss from log-probabilities and integer targets.
fn cross_entropy_loss<B: Backend>(
    log_probs: Tensor<B, 2>,
    targets: Tensor<B, 1, Int>,
    num_classes: usize,
    device: &B::Device,
) -> Tensor<B, 1> {
    // One-hot encode targets.
    let one_hot = one_hot_encode::<B>(targets, num_classes, device);

    // -sum(one_hot * log_probs) / batch_size
    let loss = (one_hot * log_probs).sum_dim(1).neg().mean();
    loss
}

/// One-hot encode integer labels to float tensor.
fn one_hot_encode<B: Backend>(
    labels: Tensor<B, 1, Int>,
    num_classes: usize,
    device: &B::Device,
) -> Tensor<B, 2> {
    let batch_size = labels.dims()[0];
    let labels_data: Vec<i64> = (0..batch_size)
        .map(|i| {
            let val: i32 = labels.clone().slice([i..i + 1]).into_scalar().elem();
            val as i64
        })
        .collect();

    let mut one_hot_data = vec![0.0f32; batch_size * num_classes];
    for (i, &label) in labels_data.iter().enumerate() {
        one_hot_data[i * num_classes + label as usize] = 1.0;
    }

    Tensor::<B, 1>::from_floats(one_hot_data.as_slice(), device)
        .reshape([batch_size, num_classes])
}

/// Evaluate accuracy on a set of samples (no augmentation).
fn evaluate_accuracy<B: Backend>(
    model: &KeywordCnn<B>,
    features: &[Vec<f32>],
    labels: &[usize],
    num_coeffs: usize,
    max_frames: usize,
    device: &B::Device,
) -> f32 {
    if features.is_empty() {
        return 0.0;
    }

    let n = features.len();
    let feature_len = num_coeffs * max_frames;
    let mut flat_input = Vec::with_capacity(n * feature_len);
    let mut flat_labels = Vec::with_capacity(n);
    for i in 0..n {
        flat_input.extend_from_slice(&features[i]);
        flat_labels.push(labels[i] as i64);
    }

    let input = Tensor::<B, 1>::from_floats(flat_input.as_slice(), device)
        .reshape([n, num_coeffs, max_frames]);
    let target = Tensor::<B, 1, Int>::from_ints(flat_labels.as_slice(), device);

    let logits = model.forward(input);
    let preds = logits.argmax(1).squeeze::<1>();
    let correct: i32 = preds.equal(target).int().sum().into_scalar().elem();

    correct as f32 / n as f32
}

/// Save the model to disk using burn's record system.
fn save_model<B: Backend>(
    model: &KeywordCnn<B>,
    model_dir: &std::path::Path,
) -> Result<()> {
    use burn::record::{FullPrecisionSettings, NamedMpkFileRecorder};

    std::fs::create_dir_all(model_dir)?;
    let recorder = NamedMpkFileRecorder::<FullPrecisionSettings>::new();
    let model_path = model_dir.join("cnn_model");
    model
        .clone()
        .save_file(model_path, &recorder)
        .map_err(|e| anyhow::anyhow!("Failed to save model: {}", e))?;
    Ok(())
}

/// Load a trained model from disk.
pub fn load_model(
    model_dir: &std::path::Path,
    num_classes: usize,
    input_channels: usize,
) -> Result<KeywordCnn<InferBackend>> {
    use burn::record::{FullPrecisionSettings, NamedMpkFileRecorder};

    let device = Default::default();
    let config = KeywordCnnConfig::new(num_classes).with_input_channels(input_channels);
    let model: KeywordCnn<InferBackend> = config.init(&device);

    let recorder = NamedMpkFileRecorder::<FullPrecisionSettings>::new();
    let model_path = model_dir.join("cnn_model");
    let model = model
        .load_file(model_path, &recorder, &device)
        .map_err(|e| anyhow::anyhow!("Failed to load model: {}", e))?;

    Ok(model)
}
