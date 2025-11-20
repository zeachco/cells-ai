use crate::math::lerp;
use macroquad::prelude::*;

pub struct Camera {
    pub x: f32,
    pub y: f32,
    pub angle: f32,
    pub target_x: f32,
    pub target_y: f32,
    pub target_angle: f32,
    pub move_speed: f32,
    pub rotation_speed: f32,
    pub lerp_factor: f32,
}

impl Camera {
    pub fn new() -> Self {
        Camera {
            x: 0.0,
            y: 0.0,
            angle: 0.0,
            target_x: 0.0,
            target_y: 0.0,
            target_angle: 0.0,
            move_speed: 200.0,
            rotation_speed: 2.0,
            lerp_factor: 0.1,
        }
    }

    pub fn handle_input(&mut self, delta_time: f32) {
        // WASD for movement
        if is_key_down(KeyCode::W) {
            self.target_y -= self.move_speed * delta_time;
        }
        if is_key_down(KeyCode::S) {
            self.target_y += self.move_speed * delta_time;
        }
        if is_key_down(KeyCode::A) {
            self.target_x -= self.move_speed * delta_time;
        }
        if is_key_down(KeyCode::D) {
            self.target_x += self.move_speed * delta_time;
        }

        // Q and E for rotation
        if is_key_down(KeyCode::Q) {
            self.target_angle -= self.rotation_speed * delta_time;
        }
        if is_key_down(KeyCode::E) {
            self.target_angle += self.rotation_speed * delta_time;
        }
    }

    pub fn update(&mut self) {
        // Smoothly interpolate position and angle towards target
        self.x = lerp(self.x, self.target_x, self.lerp_factor);
        self.y = lerp(self.y, self.target_y, self.lerp_factor);
        self.angle = lerp(self.angle, self.target_angle, self.lerp_factor);
    }
}
