use std::sync::OnceLock;

static CONFIG: OnceLock<SimulationConfig> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct SimulationConfig {
    pub world_width: f32,
    pub world_height: f32,
    pub initial_cell_count: usize,
    pub show_ui: bool,
    pub camera_tracking_speed: f32,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            world_width: 8000.0,
            world_height: 7000.0,
            initial_cell_count: 5000,
            show_ui: true,
            camera_tracking_speed: 0.5,
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
            camera_tracking_speed: 0.2,
        }
    }
}

// Check if we're running in demo mode by reading from JavaScript
fn is_demo_mode() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        unsafe extern "C" {
            fn js_is_demo_mode() -> i32;
        }

        let is_demo = unsafe { js_is_demo_mode() == 1 };
        println!("Rust: Demo mode check = {}", is_demo);
        is_demo
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        // For native builds, read from environment variable
        std::env::var("DEMO_MODE").unwrap_or_default() == "true"
    }
}

// Get the current configuration (cached after first call)
pub fn get_config() -> SimulationConfig {
    CONFIG
        .get_or_init(|| {
            let demo_mode = is_demo_mode();
            println!("Initializing config, demo_mode={}", demo_mode);

            let config = if demo_mode {
                println!("Using DEMO config");
                SimulationConfig::demo()
            } else {
                println!("Using DEFAULT config");
                SimulationConfig::default()
            };

            println!("Config initialized: {:?}", config);

            config
        })
        .clone()
}
