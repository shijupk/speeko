use burn::nn::conv::{Conv1d, Conv1dConfig};
use burn::nn::pool::AdaptiveAvgPool1d;
use burn::nn::pool::AdaptiveAvgPool1dConfig;
use burn::nn::{BatchNorm, BatchNormConfig, Dropout, DropoutConfig, Linear, LinearConfig};
use burn::prelude::*;

/// 1D CNN model for keyword classification.
///
/// Input:  [batch, 39, max_frames]  (39 MFCC+CMN+Δ/ΔΔ channels × T time steps)
/// Output: [batch, num_classes]     (log-softmax probabilities)
#[derive(Module, Debug)]
pub struct KeywordCnn<B: Backend> {
    conv1: Conv1d<B>,
    bn1: BatchNorm<B>,
    conv2: Conv1d<B>,
    bn2: BatchNorm<B>,
    conv3: Conv1d<B>,
    bn3: BatchNorm<B>,
    pool: AdaptiveAvgPool1d,
    fc1: Linear<B>,
    fc2: Linear<B>,
    dropout: Dropout,
}

/// Configuration for building a KeywordCnn.
#[derive(Config, Debug)]
pub struct KeywordCnnConfig {
    /// Number of input feature channels (e.g. 39 for MFCC+CMN+Δ/ΔΔ).
    #[config(default = 39)]
    pub input_channels: usize,
    /// Number of output classes (vocabulary size).
    pub num_classes: usize,
    /// Dropout probability.
    #[config(default = 0.3)]
    pub dropout: f64,
}

impl KeywordCnnConfig {
    /// Build the CNN model on the given device.
    pub fn init<B: Backend>(&self, device: &B::Device) -> KeywordCnn<B> {
        KeywordCnn {
            conv1: Conv1dConfig::new(self.input_channels, 64, 5)
                .with_padding(burn::nn::PaddingConfig1d::Same)
                .init(device),
            bn1: BatchNormConfig::new(64).init(device),
            conv2: Conv1dConfig::new(64, 128, 3)
                .with_padding(burn::nn::PaddingConfig1d::Same)
                .init(device),
            bn2: BatchNormConfig::new(128).init(device),
            conv3: Conv1dConfig::new(128, 128, 3)
                .with_padding(burn::nn::PaddingConfig1d::Same)
                .init(device),
            bn3: BatchNormConfig::new(128).init(device),
            pool: AdaptiveAvgPool1dConfig::new(1).init(),
            fc1: LinearConfig::new(128, 64).init(device),
            fc2: LinearConfig::new(64, self.num_classes).init(device),
            dropout: DropoutConfig::new(self.dropout).init(),
        }
    }
}

impl<B: Backend> KeywordCnn<B> {
    /// Forward pass.
    ///
    /// `input` shape: [batch, channels, time]
    /// Returns logits of shape [batch, num_classes].
    pub fn forward(&self, input: Tensor<B, 3>) -> Tensor<B, 2> {
        // Conv block 1: Conv1d -> BN -> ReLU -> MaxPool(2)
        let x = self.conv1.forward(input);
        let x = self.bn1.forward(x);
        let x = burn::tensor::activation::relu(x);
        let [b, c, t] = x.dims();
        let t_half = t / 2;
        let x = x.slice([0..b, 0..c, 0..(t_half * 2)]);
        let x = x.reshape([b, c, t_half, 2]);
        let x = x.max_dim(3).reshape([b, c, t_half]);

        // Conv block 2: Conv1d -> BN -> ReLU -> MaxPool(2)
        let x = self.conv2.forward(x);
        let x = self.bn2.forward(x);
        let x = burn::tensor::activation::relu(x);
        let [b, c, t] = x.dims();
        let t_half = t / 2;
        let x = x.slice([0..b, 0..c, 0..(t_half * 2)]);
        let x = x.reshape([b, c, t_half, 2]);
        let x = x.max_dim(3).reshape([b, c, t_half]);

        // Conv block 3: Conv1d -> BN -> ReLU
        let x = self.conv3.forward(x);
        let x = self.bn3.forward(x);
        let x = burn::tensor::activation::relu(x);

        // Global average pool -> [batch, 128, 1] -> [batch, 128]
        let x = self.pool.forward(x);
        let [b, c, _] = x.dims();
        let x = x.reshape([b, c]);

        // FC layers
        let x = self.fc1.forward(x);
        let x = burn::tensor::activation::relu(x);
        let x = self.dropout.forward(x);
        let x = self.fc2.forward(x);

        x
    }

    /// Forward pass returning log-softmax probabilities.
    pub fn forward_classification(&self, input: Tensor<B, 3>) -> Tensor<B, 2> {
        let logits = self.forward(input);
        burn::tensor::activation::log_softmax(logits, 1)
    }
}

/// Pad or truncate an MFCC sequence to a fixed number of frames.
///
/// Input:  &[Vec<f32>] of shape [num_frames][num_coefficients]
/// Output: Vec<f32> of shape [num_coefficients * max_frames] in channel-first layout
///         i.e. [C, T] flattened.
pub fn pad_or_truncate(mfcc: &[Vec<f32>], max_frames: usize) -> Vec<f32> {
    if mfcc.is_empty() {
        return vec![0.0; max_frames];
    }
    let num_coeffs = mfcc[0].len();
    let num_frames = mfcc.len();
    let mut output = vec![0.0f32; num_coeffs * max_frames];

    let frames_to_copy = num_frames.min(max_frames);

    // Layout: channel-first [C, T]
    for t in 0..frames_to_copy {
        for c in 0..num_coeffs {
            output[c * max_frames + t] = mfcc[t][c];
        }
    }

    output
}
