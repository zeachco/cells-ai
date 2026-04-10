use crate::neural_network::NeuralNetwork;
use serde::{Deserialize, Serialize};

// Expected neural network input size (must match cell sensor normalization)
// 5 sensors × 4 values + 1 energy + 5 center-of-mass values + 1 density = 27
const EXPECTED_INPUT_SIZE: usize = 27;

#[cfg(target_arch = "wasm32")]
fn key_for_tier(tier: usize) -> String {
    format!("best_brain_m{}", tier)
}

#[cfg(not(target_arch = "wasm32"))]
fn file_for_tier(tier: usize) -> String {
    format!("best_brain_m{}.json", tier)
}

// Note: The SavedState functionality has been disabled as Cell contains
// types that cannot be easily serialized (like macroquad::Color).
// Instead, we only save/load the neural network which is the key evolutionary data.

/// Wrapper struct to save the neural network with score metrics
#[derive(Serialize, Deserialize)]
struct SavedBrain {
    // Score metrics for comparison
    score: f32,
    children_count: usize,
    energy_from_cells: f32,
    age: f32,
    // The neural network itself
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

    /// Delete a key from localStorage (JavaScript implementation)
    fn storage_delete(key: *const u8, key_len: usize);
}

/// Save a neural network with score metrics to the tier-specific slot
pub fn save_best_neural_network(
    tier: usize,
    brain: &NeuralNetwork,
    generation: usize,
    score: f32,
    children_count: usize,
    energy_from_cells: f32,
    age: f32,
) {
    let saved_brain = SavedBrain {
        score,
        children_count,
        energy_from_cells,
        age,
        brain: brain.clone(),
        generation,
    };
    let json = serde_json::to_string(&saved_brain).unwrap_or_default();

    #[cfg(target_arch = "wasm32")]
    unsafe {
        let key = key_for_tier(tier);
        storage_save(key.as_ptr(), key.len(), json.as_ptr(), json.len());
        println!("💾 Best brain (tier {}) saved to localStorage", tier);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = file_for_tier(tier);
        if let Err(e) = std::fs::write(&path, json.as_bytes()) {
            println!("⚠ Failed to save brain (tier {}) to file: {}", tier, e);
        } else {
            println!("💾 Best brain (tier {}) saved to file", tier);
        }
    }
}

/// Load a neural network for the given tier slot.
/// Returns None if no saved brain exists.
/// Returns (brain, generation, score)
pub fn load_best_neural_network(tier: usize) -> Option<(NeuralNetwork, usize, f32)> {
    #[cfg(target_arch = "wasm32")]
    unsafe {
        let key = key_for_tier(tier);
        // Allocate a buffer for the result (max 1MB for neural network JSON)
        let mut buffer = vec![0u8; 1024 * 1024];
        let len = storage_load(key.as_ptr(), key.len(), buffer.as_mut_ptr(), buffer.len());

        // Try to load from localStorage first
        if len > 0 {
            buffer.truncate(len);
            if let Ok(json) = String::from_utf8(buffer) {
                if let Ok(saved_brain) = serde_json::from_str::<SavedBrain>(&json) {
                    // Validate input size matches current architecture
                    if saved_brain.brain.input_size != EXPECTED_INPUT_SIZE {
                        println!(
                            "⚠ Incompatible brain (tier {}): expected {} inputs, found {}. Deleting...",
                            tier, EXPECTED_INPUT_SIZE, saved_brain.brain.input_size
                        );
                        storage_delete(key.as_ptr(), key.len());
                        return None;
                    }
                    println!(
                        "🧠 Loaded best brain (tier {}) from localStorage (gen {}, score {:.1})",
                        tier, saved_brain.generation, saved_brain.score
                    );
                    return Some((saved_brain.brain, saved_brain.generation, saved_brain.score));
                }
                if let Some(brain) = NeuralNetwork::from_json(&json) {
                    // Validate input size for legacy format
                    if brain.input_size != EXPECTED_INPUT_SIZE {
                        println!(
                            "⚠ Incompatible legacy brain (tier {}): expected {} inputs, found {}. Deleting...",
                            tier, EXPECTED_INPUT_SIZE, brain.input_size
                        );
                        storage_delete(key.as_ptr(), key.len());
                        return None;
                    }
                    println!(
                        "🧠 Loaded best brain (tier {}) from localStorage (legacy format)",
                        tier
                    );
                    return Some((brain, 0, 0.0));
                }
            }
        }

        None
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = file_for_tier(tier);
        if let Ok(json) = std::fs::read_to_string(&path) {
            if let Ok(saved_brain) = serde_json::from_str::<SavedBrain>(&json) {
                // Validate input size matches current architecture
                if saved_brain.brain.input_size != EXPECTED_INPUT_SIZE {
                    println!(
                        "⚠ Incompatible brain (tier {}): expected {} inputs, found {}. Deleting {}...",
                        tier, EXPECTED_INPUT_SIZE, saved_brain.brain.input_size, path
                    );
                    let _ = std::fs::remove_file(&path);
                    return None;
                }
                println!(
                    "🧠 Loaded best brain (tier {}) from file (gen {}, score {:.1})",
                    tier, saved_brain.generation, saved_brain.score
                );
                return Some((saved_brain.brain, saved_brain.generation, saved_brain.score));
            }
            if let Some(brain) = NeuralNetwork::from_json(&json) {
                // Validate input size for legacy format
                if brain.input_size != EXPECTED_INPUT_SIZE {
                    println!(
                        "⚠ Incompatible legacy brain (tier {}): expected {} inputs, found {}. Deleting {}...",
                        tier, EXPECTED_INPUT_SIZE, brain.input_size, path
                    );
                    let _ = std::fs::remove_file(&path);
                    return None;
                }
                println!(
                    "🧠 Loaded best brain (tier {}) from file (legacy format)",
                    tier
                );
                return Some((brain, 0, 0.0));
            }
        }
        None
    }
}
