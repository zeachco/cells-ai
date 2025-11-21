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

            // Inherited Attributes
            color: Color::new(
                rand::gen_range(0.5, 1.0),
                rand::gen_range(0.5, 1.0),
                rand::gen_range(0.5, 1.0),
                1.0,
            ),
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

        Cell {
            // Individual State
            x: self.x + angle.cos() * offset,
            y: self.y + angle.sin() * offset,
            velocity_x: angle.cos() * self.speed * rand::gen_range(0.5, 1.0),
            velocity_y: angle.sin() * self.speed * rand::gen_range(0.5, 1.0),
            energy: 0.0, // Will be set by caller
            angle: rand::gen_range(0.0, std::f32::consts::TAU),
            angle_velocity: rand::gen_range(-0.05, 0.05),
            state: CellState::Alive,
            age: 0.0, // Start as newborn

            // Inherited Attributes (from parent)
            color: self.color,
            radius: rand::gen_range(6.0, 15.0),
            move_probability: rand::gen_range(0.05, 0.15),
            turn_probability: rand::gen_range(0.05, 0.15),
            speed: rand::gen_range(1.0, 3.0),
            turn_rate: rand::gen_range(0.05, 0.15),
            energy_chunk_size: self.energy_chunk_size,
            species_multiplier: self.species_multiplier,
            mass: self.mass,
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

            // Only allow active movement if cell is alive
            if rand::gen_range(0.0, 1.0) < self.move_probability {
                self.forward();
            }
            if rand::gen_range(0.0, 1.0) < self.turn_probability {
                self.random_turn();
            }
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

    pub fn random_turn(&mut self) {
        let rand_val = rand::gen_range(0.0, 1.0);
        if rand_val < 0.33 {
            self.turn_left();
        } else if rand_val < 0.66 {
            self.turn_right();
        }
        // Otherwise, do nothing (33% chance)
    }
}
