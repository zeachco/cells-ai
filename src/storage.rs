use crate::cell::Cell;
use macroquad::file::load_string;
use serde::{Deserialize, Serialize};

const SAVE_FILE: &str = "cells_best.json";

#[derive(Serialize, Deserialize)]
pub struct SavedState {
    pub best_cell: Cell,
    pub best_energy: f32,
    pub version: u32, // For future compatibility
}

impl SavedState {
    pub fn new(best_cell: Cell, best_energy: f32) -> Self {
        Self {
            best_cell,
            best_energy,
            version: 1,
        }
    }
}

/// Save the best cell to storage
/// Native: saves to filesystem, Web: saves to localStorage
pub async fn save_best_cell(cell: &Cell, energy: f32) -> Result<(), String> {
    let state = SavedState::new(cell.clone(), energy);

    match serde_json::to_string_pretty(&state) {
        Ok(json) => {
            // Use platform-specific storage
            #[cfg(not(target_arch = "wasm32"))]
            {
                // Native: write to file
                std::fs::write(SAVE_FILE, json.as_bytes())
                    .map_err(|e| format!("Failed to save: {}", e))?;
            }

            #[cfg(target_arch = "wasm32")]
            {
                // Web: use localStorage via web_sys
                if let Some(window) = web_sys::window()
                    && let Ok(Some(storage)) = window.local_storage()
                {
                    let _ = storage.set_item(SAVE_FILE, &json);
                }
            }

            println!("✓ Best cell saved (energy: {:.1})", energy);
            Ok(())
        }
        Err(e) => Err(format!("Failed to serialize: {}", e)),
    }
}

/// Load the best cell from storage
/// Returns None if no save exists or if loading fails
pub async fn load_best_cell() -> Option<SavedState> {
    match load_string(SAVE_FILE).await {
        Ok(contents) => match serde_json::from_str::<SavedState>(&contents) {
            Ok(state) => {
                println!("✓ Loaded saved cell (energy: {:.1})", state.best_energy);
                Some(state)
            }
            Err(e) => {
                println!("⚠ Failed to parse save file: {}", e);
                None
            }
        },
        Err(_) => {
            println!("ℹ No save file found, starting fresh");
            None
        }
    }
}
