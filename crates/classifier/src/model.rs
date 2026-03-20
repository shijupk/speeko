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
        // Simple stride-2 downsampling via slicing (max pool 1d with kernel 2, stride 2)
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
        return vec![0.0; max_frames]; // degenerate
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
    // Remaining positions are already zero-padded.

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use burn::backend::NdArray;

    type TestBackend = NdArray<f32>;

    #[test]
    fn test_model_forward_shape() {
        let device = Default::default();
        let config = KeywordCnnConfig::new(10);
        let model: KeywordCnn<TestBackend> = config.init(&device);

        // batch=2, channels=39, time=100
        let input = Tensor::<TestBackend, 3>::zeros([2, 39, 100], &device);
        let output = model.forward(input);
        let [b, classes] = output.dims();
        assert_eq!(b, 2);
        assert_eq!(classes, 10);
    }

    #[test]
    fn test_pad_or_truncate_pad() {
        let mfcc = vec![vec![1.0, 2.0], vec![3.0, 4.0]]; // 2 frames, 2 coeffs
        let result = pad_or_truncate(&mfcc, 4);
        // Channel-first: [C=2, T=4]
        // c=0: [1.0, 3.0, 0.0, 0.0]
        // c=1: [2.0, 4.0, 0.0, 0.0]
        assert_eq!(result.len(), 8);
        assert_eq!(result[0], 1.0); // c=0, t=0
        assert_eq!(result[1], 3.0); // c=0, t=1
        assert_eq!(result[2], 0.0); // c=0, t=2 (padded)
        assert_eq!(result[4], 2.0); // c=1, t=0
        assert_eq!(result[5], 4.0); // c=1, t=1
    }

    #[test]
    fn test_pad_or_truncate_truncate() {
        let mfcc = vec![vec![1.0], vec![2.0], vec![3.0], vec![4.0]]; // 4 frames, 1 coeff
        let result = pad_or_truncate(&mfcc, 2);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], 1.0);
        assert_eq!(result[1], 2.0);
    }
}
