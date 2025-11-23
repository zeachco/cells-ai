use crate::neural_network::NeuralNetwork;
use crate::world::{WORLD_HEIGHT, WORLD_WIDTH};
use macroquad::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CellState {
    Alive,
    Corpse,
}

pub struct Cell {
    // ===== Individual State (not inherited) =====
    pub x: f32,
    pub y: f32,
    pub velocity_x: f32,
    pub velocity_y: f32,
    pub energy: f32,
    pub angle: f32,
    pub angle_velocity: f32,
    pub state: CellState,
    pub age: f32, // 0 to 100+, affects energy costs and size

    // ===== Sensors =====
    // Each sensor returns: (cell_index, angle_from_front, distance, mass, is_alive)
    // angle_from_front: -180..180 degrees relative to cell's facing direction
    // distance: 0..200 units
    // mass: target cell's mass (energy capacity)
    // is_alive: 1.0 if alive, 0.0 if dead/corpse
    pub nearest_cells: Vec<(usize, f32, f32, f32, f32)>, // (index, angle, distance, mass, is_alive) for 5 nearest cells

    // ===== Neural Network Brain =====
    pub brain: NeuralNetwork,

    // ===== Inherited Attributes (passed to children) =====
    pub color: Color,
    pub radius: f32, // Base radius (full size)
    pub move_probability: f32,
    pub turn_probability: f32,
    pub speed: f32,
    pub turn_rate: f32,
    pub energy_chunk_size: f32,
    pub species_multiplier: f32,
    pub mass: f32, // Max energy capacity
}

impl Cell {
    // Apply 1% mutation variance to a value, clamped to min/max range
    fn mutate(value: f32, min: f32, max: f32) -> f32 {
        let variance = rand::gen_range(-0.01, 0.01); // ±1%
        let mutated = value * (1.0 + variance);
        mutated.clamp(min, max)
    }

    // Convert RGB to HSV
    fn rgb_to_hsv(color: Color) -> (f32, f32, f32) {
        let r = color.r;
        let g = color.g;
        let b = color.b;

        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;

        let h = if delta == 0.0 {
            0.0
        } else if max == r {
            60.0 * (((g - b) / delta) % 6.0)
        } else if max == g {
            60.0 * (((b - r) / delta) + 2.0)
        } else {
            60.0 * (((r - g) / delta) + 4.0)
        };

        let s = if max == 0.0 { 0.0 } else { delta / max };
        let v = max;

        (h, s, v)
    }

    // Convert HSV to RGB
    fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Color {
        let c = v * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = v - c;

        let (r, g, b) = if h < 60.0 {
            (c, x, 0.0)
        } else if h < 120.0 {
            (x, c, 0.0)
        } else if h < 180.0 {
            (0.0, c, x)
        } else if h < 240.0 {
            (0.0, x, c)
        } else if h < 300.0 {
            (x, 0.0, c)
        } else {
            (c, 0.0, x)
        };

        Color::new(r + m, g + m, b + m, 1.0)
    }

    // Get current radius based on age
    // Age 0-30: scales from 10% to 100% of base radius
    // Age 30+: stays at 100%
    pub fn get_current_radius(&self) -> f32 {
        if self.age < 30.0 {
            let size_percent = 0.1 + (self.age / 30.0) * 0.9; // 10% to 100%
            self.radius * size_percent
        } else {
            self.radius
        }
    }

    // Get age-based energy cost multiplier
    // Age 0-100: costs scale from 1.0x to 2.0x
    fn get_age_cost_multiplier(&self) -> f32 {
        1.0 + (self.age / 100.0).min(1.0)
    }

    pub fn spawn() -> Self {
        let speed = rand::gen_range(1.0, 3.0);
        let angle = rand::gen_range(0.0, std::f32::consts::TAU);
        // Energy chunk size: 50 ± 10% = 45 to 55
        let energy_chunk_size = rand::gen_range(45.0, 55.0);
        // Species multiplier: how efficiently energy is extracted (0.9x to 2.0x)
        let species_multiplier = rand::gen_range(0.9, 2.0);
        // Mass: max energy capacity, around 200 ± 10%
        let mass = rand::gen_range(180.0, 220.0);

        // Create neural network: 20 inputs (5 sensors × 4 values), 4 outputs (actions)
        let brain = NeuralNetwork::new(20, 4);

        Cell {
            // Individual State
            x: rand::gen_range(0.0, WORLD_WIDTH),
            y: rand::gen_range(0.0, WORLD_HEIGHT),
            velocity_x: angle.cos() * speed * rand::gen_range(0.5, 1.0),
            velocity_y: angle.sin() * speed * rand::gen_range(0.5, 1.0),
            energy: 100.0,
            angle,
            angle_velocity: rand::gen_range(-0.05, 0.05),
            state: CellState::Alive,
            age: 0.0,

            // Sensors
            nearest_cells: Vec::new(),

            // Neural Network Brain
            brain,

            // Inherited Attributes
            color: Self::hsv_to_rgb(180.0, 0.8, 0.9), // Teal color (hue=180°)
            radius: rand::gen_range(6.0, 15.0),
            move_probability: rand::gen_range(0.05, 0.15),
            turn_probability: rand::gen_range(0.05, 0.15),
            speed,
            turn_rate: rand::gen_range(0.05, 0.15),
            energy_chunk_size,
            species_multiplier,
            mass,
        }
    }

    pub fn spawn_child(&self) -> Self {
        let angle = rand::gen_range(0.0, std::f32::consts::TAU);
        let offset = 15.0;

        // Apply mutation to inherited attributes with their respective ranges
        let mutated_speed = Self::mutate(self.speed, 1.0, 3.0);

        // Mutate color by adjusting hue angle (±1%)
        let (h, s, v) = Self::rgb_to_hsv(self.color);
        let hue_variance = rand::gen_range(-0.01, 0.01); // ±1%
        let mutated_hue = (h + h * hue_variance).rem_euclid(360.0); // Wrap around at 360°
        let mutated_color = Self::hsv_to_rgb(mutated_hue, s, v);

        // Clone and mutate the parent's brain
        // Mutation rate varies randomly between 1% and 10%
        let mutation_rate = rand::gen_range(0.01, 0.10);
        let mut brain = self.brain.clone();
        brain.mutate(mutation_rate);

        Cell {
            // Individual State
            x: self.x + angle.cos() * offset,
            y: self.y + angle.sin() * offset,
            velocity_x: angle.cos() * mutated_speed * rand::gen_range(0.5, 1.0),
            velocity_y: angle.sin() * mutated_speed * rand::gen_range(0.5, 1.0),
            energy: 0.0, // Will be set by caller
            angle: rand::gen_range(0.0, std::f32::consts::TAU),
            angle_velocity: rand::gen_range(-0.05, 0.05),
            state: CellState::Alive,
            age: 0.0, // Start as newborn

            // Sensors
            nearest_cells: Vec::new(),

            // Neural Network Brain (inherited and mutated)
            brain,

            // Inherited Attributes (from parent with 1% mutation)
            color: mutated_color,
            radius: Self::mutate(self.radius, 6.0, 15.0),
            move_probability: Self::mutate(self.move_probability, 0.05, 0.15),
            turn_probability: Self::mutate(self.turn_probability, 0.05, 0.15),
            speed: mutated_speed,
            turn_rate: Self::mutate(self.turn_rate, 0.05, 0.15),
            energy_chunk_size: Self::mutate(self.energy_chunk_size, 45.0, 55.0),
            species_multiplier: Self::mutate(self.species_multiplier, 0.9, 2.0),
            mass: Self::mutate(self.mass, 180.0, 220.0),
        }
    }

    // Normalize sensor inputs for neural network
    // Each sensor returns 4 values: angle, distance, mass, is_alive
    // Total: 5 sensors × 4 values = 20 inputs
    fn normalize_sensors(&self) -> Vec<f32> {
        let max_sensor_range = 200.0; // Match the sensor range from world.rs
        let max_mass = 220.0; // Maximum mass value from spawn()
        let mut inputs = Vec::with_capacity(20);

        for i in 0..5 {
            if i < self.nearest_cells.len() {
                let (_index, angle, distance, mass, is_alive) = self.nearest_cells[i];

                // Angle: -PI..PI -> -1..1
                let normalized_angle = angle / std::f32::consts::PI;

                // Distance: 0..200 -> -1..1 (closer = higher value)
                let normalized_distance = (max_sensor_range - distance) / max_sensor_range;
                let normalized_distance = normalized_distance * 2.0 - 1.0;

                // Mass: 0..220 -> -1..1 (normalized around expected range)
                let normalized_mass = (mass / max_mass) * 2.0 - 1.0;

                // Is alive: 0 or 1 -> -1 or 1 (dead = -1, alive = 1)
                let normalized_alive = is_alive * 2.0 - 1.0;

                inputs.push(normalized_angle);
                inputs.push(normalized_distance);
                inputs.push(normalized_mass);
                inputs.push(normalized_alive);
            } else {
                // No cell detected in this sensor slot
                // Push default values (-1 for "nothing detected")
                inputs.push(-1.0); // angle
                inputs.push(-1.0); // distance (far away)
                inputs.push(-1.0); // mass (no target)
                inputs.push(-1.0); // is_alive (no target = dead)
            }
        }

        inputs
    }

    // Make a decision using the neural network
    // Actions: 0 = no-op, 1 = turn_left, 2 = turn_right, 3 = forward
    fn decide_action(&mut self) {
        let inputs = self.normalize_sensors();
        let action = self.brain.get_best_action(&inputs);

        match action {
            0 => {} // No-op (do nothing)
            1 => self.turn_left(),
            2 => self.turn_right(),
            3 => self.forward(),
            _ => {} // Should never happen, but handle gracefully
        }
    }

    pub fn update(&mut self) {
        // State transition: Alive -> Corpse when energy depleted
        if self.state == CellState::Alive && self.energy <= 0.0 {
            self.state = CellState::Corpse;
        }

        // Increment age for alive cells (0.1 per tick, reaches 100 in ~1000 ticks)
        if self.state == CellState::Alive {
            self.age += 0.1;
        }

        // Cap energy at mass (max capacity)
        if self.energy > self.mass {
            self.energy = self.mass;
        }

        // Passive energy loss for all cells
        if self.state == CellState::Alive {
            // Alive cells lose energy slowly (metabolism)
            self.energy -= 0.03;

            // Use neural network to decide action instead of random movement
            self.decide_action();
        } else if self.state == CellState::Corpse {
            // Corpse decay: lose 0.02 energy per tick
            self.energy -= 0.02;
        }

        // Apply mass-based velocity slowdown
        // Higher mass = slower movement (mass acts as inertia/drag)
        let mass_factor = 200.0 / self.mass; // Normalize around 200
        let slowdown = mass_factor.max(0.5); // Don't slow down too much

        // Always apply velocity (drifting continues even when dead)
        self.x += self.velocity_x * slowdown;
        self.y += self.velocity_y * slowdown;
        self.angle += self.angle_velocity;

        self.velocity_y *= 0.95; // Friction
        self.velocity_x *= 0.95; // Friction
        self.angle_velocity *= 0.9; // Rotational friction
    }

    // Called when cell gains energy (from feeding)
    // For young cells (age < 20), energy is lost to growth
    pub fn gain_energy(&mut self, amount: f32) {
        if self.age < 20.0 {
            // Young cells: energy goes to growth, not stored
            // Energy is simply discarded (used for growing)
            return;
        }

        // Mature cells: energy is stored
        self.energy += amount;
    }

    pub fn render(&self, camera_x: f32, camera_y: f32) {
        let screen_x = self.x - camera_x;
        let screen_y = self.y - camera_y;
        let current_radius = self.get_current_radius();

        // Viewport culling: only render if cell is visible on screen
        let margin = current_radius * 1.5; // Account for direction line
        let screen_w = screen_width();
        let screen_h = screen_height();

        if screen_x < -margin
            || screen_x > screen_w + margin
            || screen_y < -margin
            || screen_y > screen_h + margin
        {
            return; // Cell is outside viewport, skip rendering
        }

        // Draw the cell body
        if self.state == CellState::Alive {
            // Alive cells are filled with their color
            draw_circle(screen_x, screen_y, current_radius, self.color);
        } else {
            // Corpse cells are grayed out (reduced saturation and brightness)
            let gray_color = Color::new(
                self.color.r * 0.3,
                self.color.g * 0.3,
                self.color.b * 0.3,
                self.color.a,
            );
            draw_circle_lines(screen_x, screen_y, current_radius, 2.0, gray_color);
        }

        // Draw a line showing the direction the cell is facing (only for alive cells)
        if self.state == CellState::Alive {
            let line_length = current_radius * 1.5;
            let end_x = screen_x + self.angle.cos() * line_length;
            let end_y = screen_y + self.angle.sin() * line_length;
            draw_line(screen_x, screen_y, end_x, end_y, 2.0, WHITE);
        }
    }

    pub fn turn_left(&mut self) {
        let age_multiplier = self.get_age_cost_multiplier();
        let cost = 0.45 * age_multiplier; // 0.5 reduced by 10%
        if self.energy >= cost {
            self.angle_velocity -= self.turn_rate;
            self.energy -= cost;
        }
    }

    pub fn turn_right(&mut self) {
        let age_multiplier = self.get_age_cost_multiplier();
        let cost = 0.45 * age_multiplier; // 0.5 reduced by 10%
        if self.energy >= cost {
            self.angle_velocity += self.turn_rate;
            self.energy -= cost;
        }
    }

    pub fn forward(&mut self) {
        let age_multiplier = self.get_age_cost_multiplier();
        let cost = 0.9 * age_multiplier; // 1.0 reduced by 10%
        if self.energy >= cost {
            self.velocity_x = self.angle.cos() * self.speed;
            self.velocity_y = self.angle.sin() * self.speed;
            self.energy -= cost;
        }
    }
}
