use crate::neural_network::NeuralNetwork;
use serde::{Deserialize, Serialize};

const NEURAL_NETWORK_KEY: &str = "cells_best_brain";

// Note: The SavedState functionality has been disabled as Cell contains
// types that cannot be easily serialized (like macroquad::Color).
// Instead, we only save/load the neural network which is the key evolutionary data.

/// Wrapper struct to save both the neural network and generation count
#[derive(Serialize, Deserialize)]
struct SavedBrain {
    brain: NeuralNetwork,
    generation: usize,
}

#[cfg(target_arch = "wasm32")]
unsafe extern "C" {
    /// Save a string to localStorage (JavaScript implementation)
    fn storage_save(key: *const u8, key_len: usize, value: *const u8, value_len: usize);

    /// Load a string from localStorage (JavaScript implementation)
    /// Returns the length of the loaded string, or 0 if not found
    fn storage_load(key: *const u8, key_len: usize, buffer: *mut u8, buffer_len: usize) -> usize;
}

/// Save a neural network and generation to localStorage
/// This is called each time the best cell reproduces
pub fn save_best_neural_network(brain: &NeuralNetwork, generation: usize) {
    let saved_brain = SavedBrain {
        brain: brain.clone(),
        generation,
    };
    let json = serde_json::to_string(&saved_brain).unwrap_or_default();

    #[cfg(target_arch = "wasm32")]
    unsafe {
        storage_save(
            NEURAL_NETWORK_KEY.as_ptr(),
            NEURAL_NETWORK_KEY.len(),
            json.as_ptr(),
            json.len(),
        );
        println!("💾 Best brain saved to localStorage");
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        // For native builds, save to file
        if let Err(e) = std::fs::write("cells_best_brain.json", json.as_bytes()) {
            println!("⚠ Failed to save brain to file: {}", e);
        } else {
            println!("💾 Best brain saved to file");
        }
    }
}

/// Load a neural network and generation from localStorage
/// Returns None if no saved brain exists
pub fn load_best_neural_network() -> Option<(NeuralNetwork, usize)> {
    #[cfg(target_arch = "wasm32")]
    unsafe {
        // Allocate a buffer for the result (max 1MB for neural network JSON)
        let mut buffer = vec![0u8; 1024 * 1024];
        let len = storage_load(
            NEURAL_NETWORK_KEY.as_ptr(),
            NEURAL_NETWORK_KEY.len(),
            buffer.as_mut_ptr(),
            buffer.len(),
        );

        if len > 0 {
            buffer.truncate(len);
            if let Ok(json) = String::from_utf8(buffer) {
                // Try to load as SavedBrain first (new format)
                if let Ok(saved_brain) = serde_json::from_str::<SavedBrain>(&json) {
                    println!(
                        "🧠 Loaded best brain from localStorage (generation {})",
                        saved_brain.generation
                    );
                    return Some((saved_brain.brain, saved_brain.generation));
                }
                // Fall back to old format (just NeuralNetwork)
                if let Some(brain) = NeuralNetwork::from_json(&json) {
                    println!(
                        "🧠 Loaded best brain from localStorage (legacy format, generation unknown)"
                    );
                    return Some((brain, 0));
                }
            }
        }
        None
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        // For native builds, load from file
        if let Ok(json) = std::fs::read_to_string("cells_best_brain.json") {
            // Try to load as SavedBrain first (new format)
            if let Ok(saved_brain) = serde_json::from_str::<SavedBrain>(&json) {
                println!(
                    "🧠 Loaded best brain from file (generation {})",
                    saved_brain.generation
                );
                return Some((saved_brain.brain, saved_brain.generation));
            }
            // Fall back to old format (just NeuralNetwork)
            if let Some(brain) = NeuralNetwork::from_json(&json) {
                println!("🧠 Loaded best brain from file (legacy format, generation unknown)");
                return Some((brain, 0));
            }
        }
        None
    }
}
