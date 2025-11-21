use crate::world::{WORLD_HEIGHT, WORLD_WIDTH};
use macroquad::prelude::*;

pub struct Cell {
    pub x: f32,
    pub y: f32,
    pub velocity_x: f32,
    pub velocity_y: f32,
    pub energy: f32,
    // pub stability: f32,
    pub angle: f32,
    pub angle_velocity: f32,
    pub color: Color,
}

impl Cell {
    pub fn spawn() -> Self {
        Cell {
            x: rand::gen_range(0.0, WORLD_WIDTH),
            y: rand::gen_range(0.0, WORLD_HEIGHT),
            velocity_x: 0.0,
            velocity_y: 0.0,
            energy: 100.0,
            // stability: 1.0,
            angle: rand::gen_range(0.0, std::f32::consts::TAU),
            angle_velocity: 0.0,
            color: Color::new(
                rand::gen_range(0.5, 1.0),
                rand::gen_range(0.5, 1.0),
                rand::gen_range(0.5, 1.0),
                1.0,
            ),
        }
    }

    pub fn update(&mut self) {
        // Only allow active movement if cell has energy
        if self.energy > 0.0 {
            if rand::gen_range(0.0, 1.0) < 0.1 {
                self.forward();
            }
            if rand::gen_range(0.0, 1.0) < 0.1 {
                self.random_turn();
            }
        }

        // Always apply velocity (drifting continues even when dead)
        self.x += self.velocity_x;
        self.y += self.velocity_y;
        self.angle += self.angle_velocity;

        self.velocity_y *= 0.95; // Friction
        self.velocity_x *= 0.95; // Friction
        self.angle_velocity *= 0.9; // Rotational friction
    }

    pub fn render(&self, camera_x: f32, camera_y: f32) {
        let radius = 10.0;
        let screen_x = self.x - camera_x;
        let screen_y = self.y - camera_y;

        // Draw the cell body
        if self.energy > 0.0 {
            // Alive cells are filled
            draw_circle(screen_x, screen_y, radius, self.color);
        } else {
            // Dead cells are just strokes (outline only)
            draw_circle_lines(screen_x, screen_y, radius, 2.0, self.color);
        }

        // Draw a line showing the direction the cell is facing (only for alive cells)
        if self.energy > 0.0 {
            let line_length = radius * 1.5;
            let end_x = screen_x + self.angle.cos() * line_length;
            let end_y = screen_y + self.angle.sin() * line_length;
            draw_line(screen_x, screen_y, end_x, end_y, 2.0, WHITE);
        }
    }

    pub fn turn_left(&mut self) {
        if self.energy >= 0.5 {
            self.angle_velocity -= 0.1;
            self.energy -= 0.5;
        }
    }

    pub fn turn_right(&mut self) {
        if self.energy >= 0.5 {
            self.angle_velocity += 0.1;
            self.energy -= 0.5;
        }
    }

    pub fn forward(&mut self) {
        if self.energy >= 1.0 {
            let speed = 2.0;
            self.velocity_x = self.angle.cos() * speed;
            self.velocity_y = self.angle.sin() * speed;
            self.energy -= 1.0;
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
