use macroquad::prelude::rand;
use serde::{Deserialize, Serialize};

/// Neural network for cell decision-making
///
/// Architecture:
/// - Inputs: 5 sensors (distances to nearest cells)
/// - Hidden layer: 2 * (inputs + outputs) nodes
/// - Outputs: 4 actions (no-op, turn_left, turn_right, forward)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuralNetwork {
    // Input to hidden layer weights [hidden_size x input_size]
    weights_ih: Vec<Vec<f32>>,
    // Hidden layer biases [hidden_size]
    bias_h: Vec<f32>,
    // Hidden to output layer weights [output_size x hidden_size]
    weights_ho: Vec<Vec<f32>>,
    // Output layer biases [output_size]
    bias_o: Vec<f32>,

    pub input_size: usize,
    pub hidden_size: usize,
    pub output_size: usize,
}

impl NeuralNetwork {
    /// Create a new neural network with random weights (1x hidden layer width)
    ///
    /// # Arguments
    /// * `input_size` - Number of inputs (proximity sensors)
    /// * `output_size` - Number of outputs (actions)
    #[allow(dead_code)]
    pub fn new(input_size: usize, output_size: usize) -> Self {
        Self::new_with_multiplier(input_size, output_size, 1)
    }

    /// Create a new neural network with a hidden layer size multiplier
    ///
    /// # Arguments
    /// * `input_size` - Number of inputs
    /// * `output_size` - Number of outputs
    /// * `hidden_multiplier` - Multiplier for hidden layer width (1x, 2x, 3x, 4x)
    pub fn new_with_multiplier(
        input_size: usize,
        output_size: usize,
        hidden_multiplier: usize,
    ) -> Self {
        let hidden_size = hidden_multiplier * 2 * (input_size + output_size);

        // Initialize weights and biases with random values between -1.0 and 1.0
        let weights_ih = (0..hidden_size)
            .map(|_| {
                (0..input_size)
                    .map(|_| rand::gen_range(-1.0, 1.0))
                    .collect()
            })
            .collect();

        let bias_h = (0..hidden_size)
            .map(|_| rand::gen_range(-1.0, 1.0))
            .collect();

        let weights_ho = (0..output_size)
            .map(|_| {
                (0..hidden_size)
                    .map(|_| rand::gen_range(-1.0, 1.0))
                    .collect()
            })
            .collect();

        let bias_o = (0..output_size)
            .map(|_| rand::gen_range(-1.0, 1.0))
            .collect();

        NeuralNetwork {
            weights_ih,
            bias_h,
            weights_ho,
            bias_o,
            input_size,
            hidden_size,
            output_size,
        }
    }

    /// Forward pass through the network
    ///
    /// # Arguments
    /// * `inputs` - Sensor values (should be normalized)
    ///
    /// # Returns
    /// Vector of output activations (one per action)
    pub fn forward(&self, inputs: &[f32]) -> Vec<f32> {
        assert_eq!(inputs.len(), self.input_size, "Input size mismatch");

        // Compute hidden layer activations
        let mut hidden = vec![0.0; self.hidden_size];
        for (i, hidden_val) in hidden.iter_mut().enumerate().take(self.hidden_size) {
            let mut sum = self.bias_h[i];
            for (j, &input) in inputs.iter().enumerate().take(self.input_size) {
                sum += self.weights_ih[i][j] * input;
            }
            *hidden_val = Self::relu(sum);
        }

        // Compute output layer activations
        let mut outputs = vec![0.0; self.output_size];
        for (i, output_val) in outputs.iter_mut().enumerate().take(self.output_size) {
            let mut sum = self.bias_o[i];
            for (j, &hidden_val) in hidden.iter().enumerate().take(self.hidden_size) {
                sum += self.weights_ho[i][j] * hidden_val;
            }
            *output_val = sum; // No activation on output (will use softmax or argmax)
        }

        outputs
    }

    /// ReLU activation function
    fn relu(x: f32) -> f32 {
        x.max(0.0)
    }

    /// Mutate the neural network weights and biases
    ///
    /// # Arguments
    /// * `rate` - Mutation rate (0.0 to 1.0), represents the probability that each weight will be mutated
    ///
    /// When a weight is mutated, it's adjusted by a random value in the range [-0.1, 0.1]
    pub fn mutate(&mut self, rate: f32) {
        let rate = rate.clamp(0.0, 1.0);

        // Mutate input-to-hidden weights
        for i in 0..self.hidden_size {
            for j in 0..self.input_size {
                if rand::gen_range(0.0, 1.0) < rate {
                    let delta = rand::gen_range(-0.1, 0.1);
                    self.weights_ih[i][j] = (self.weights_ih[i][j] + delta).clamp(-2.0, 2.0);
                }
            }
        }

        // Mutate hidden biases
        for i in 0..self.hidden_size {
            if rand::gen_range(0.0, 1.0) < rate {
                let delta = rand::gen_range(-0.1, 0.1);
                self.bias_h[i] = (self.bias_h[i] + delta).clamp(-2.0, 2.0);
            }
        }

        // Mutate hidden-to-output weights
        for i in 0..self.output_size {
            for j in 0..self.hidden_size {
                if rand::gen_range(0.0, 1.0) < rate {
                    let delta = rand::gen_range(-0.1, 0.1);
                    self.weights_ho[i][j] = (self.weights_ho[i][j] + delta).clamp(-2.0, 2.0);
                }
            }
        }

        // Mutate output biases
        for i in 0..self.output_size {
            if rand::gen_range(0.0, 1.0) < rate {
                let delta = rand::gen_range(-0.1, 0.1);
                self.bias_o[i] = (self.bias_o[i] + delta).clamp(-2.0, 2.0);
            }
        }
    }

    /// Number of multiplications performed in one forward pass
    pub fn operation_count(&self) -> usize {
        self.hidden_size * (self.input_size + self.output_size)
    }

    /// Get the action index with the highest activation
    pub fn get_best_action(&self, inputs: &[f32]) -> usize {
        let outputs = self.forward(inputs);
        outputs
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(idx, _)| idx)
            .unwrap_or(0)
    }

    /// Serialize the neural network to JSON
    /// Note: Currently unused - the codebase serializes SavedBrain structs directly.
    /// Kept for API symmetry with from_json() which is used for legacy format migration.
    #[allow(dead_code)]
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Deserialize the neural network from JSON
    /// Used in storage.rs for backward compatibility with legacy save formats.
    pub fn from_json(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_neural_network_creation() {
        let nn = NeuralNetwork::new(5, 4);
        assert_eq!(nn.input_size, 5);
        assert_eq!(nn.output_size, 4);
        assert_eq!(nn.hidden_size, 18); // 2 * (5 + 4)
    }

    #[test]
    fn test_forward_pass() {
        let nn = NeuralNetwork::new(5, 4);
        let inputs = vec![0.1, 0.2, 0.3, 0.4, 0.5];
        let outputs = nn.forward(&inputs);
        assert_eq!(outputs.len(), 4);
    }

    #[test]
    fn test_mutate() {
        let mut nn = NeuralNetwork::new(5, 4);
        let original_weights = nn.weights_ih.clone();
        nn.mutate(1.0); // 100% mutation rate
        // At least some weights should have changed
        let changed = nn
            .weights_ih
            .iter()
            .zip(original_weights.iter())
            .any(|(a, b)| a != b);
        assert!(changed);
    }

    #[test]
    fn test_get_best_action() {
        let nn = NeuralNetwork::new(5, 4);
        let inputs = vec![0.1, 0.2, 0.3, 0.4, 0.5];
        let action = nn.get_best_action(&inputs);
        assert!(action < 4);
    }

    #[test]
    fn test_get_best_action_nan() {
        let mut nn = NeuralNetwork::new(1, 1);
        // hidden_size = 2 * (1 + 1) = 4
        // Set weights to zero to avoid random noise interference
        nn.weights_ih = vec![vec![0.0]; 4]; // 4 rows (hidden neurons), 1 column (inputs)
        nn.bias_h = vec![0.0; 4]; // 4 hidden neurons
        nn.weights_ho = vec![vec![0.0; 4]]; // 1 row (outputs), 4 columns (hidden neurons)
        nn.bias_o = vec![0.0];

        // This input should result in NaN after some operations in a more complex network,
        // but here we just want to test if get_best_action panics when forward() returns NaN.
        // We can't easily force forward() to return NaN without changing it,
        // but we can mock forward's results if we could, but let's just make a test that
        // specifically triggers the partial_cmp(NaN) case if we can.

        // Actually, let's just test get_best_action directly by injecting NaN in inputs
        // and hoping it propagates. ReLU(NaN) is NaN in some implementations, but here
        // it is x.max(0.0). NaN.max(0.0) is 0.0 in Rust!
        // So we need to find another way to get NaN in outputs.

        // If we set a weight to NaN, it will propagate.
        nn.weights_ho[0][0] = f32::NAN;
        let inputs = vec![1.0];
        // forward will compute: sum = bias_o[0] + weights_ho[0][0] * hidden[0]
        // sum = 0.0 + NaN * ReLU(0.0 + 0.0 * 1.0) = NaN
        let action = nn.get_best_action(&inputs);
        assert_eq!(action, 0); // Should not panic and return the only available action
    }
}
