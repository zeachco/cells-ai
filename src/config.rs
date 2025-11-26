use std::sync::Mutex;

// Global configuration that can be set from JavaScript
static CONFIG: Mutex<Option<SimulationConfig>> = Mutex::new(None);

#[derive(Debug, Clone)]
pub struct SimulationConfig {
    pub world_width: f32,
    pub world_height: f32,
    pub initial_cell_count: usize,
    pub show_ui: bool,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            world_width: 8000.0,
            world_height: 7000.0,
            initial_cell_count: 1000,
            show_ui: true,
        }
    }
}

impl SimulationConfig {
    pub fn demo() -> Self {
        Self {
            world_width: 4000.0,
            world_height: 3000.0,
            initial_cell_count: 1000,
            show_ui: false,
        }
    }
}

// Set configuration (called from JavaScript)
#[unsafe(no_mangle)]
pub extern "C" fn set_demo_mode(enabled: bool) {
    let config = if enabled {
        SimulationConfig::demo()
    } else {
        SimulationConfig::default()
    };

    *CONFIG.lock().unwrap() = Some(config);
}

// Get the current configuration
pub fn get_config() -> SimulationConfig {
    CONFIG.lock().unwrap().clone().unwrap_or_default()
}
