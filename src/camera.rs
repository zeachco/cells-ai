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
    // Drag state
    is_dragging: bool,
    last_mouse_x: f32,
    last_mouse_y: f32,
    last_drag_delta_x: f32,
    last_drag_delta_y: f32,
    last_scroll_delta_x: f32,
    last_scroll_delta_y: f32,
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
            move_speed: 2000.0, // Increased from 1000.0
            rotation_speed: 2.0,
            lerp_factor: 0.15, // Increased from 0.05 for more friction
            is_dragging: false,
            last_mouse_x: 0.0,
            last_mouse_y: 0.0,
            last_drag_delta_x: 0.0,
            last_drag_delta_y: 0.0,
            last_scroll_delta_x: 0.0,
            last_scroll_delta_y: 0.0,
        }
    }

    pub fn handle_input(&mut self, delta_time: f32, skip_mouse_input: bool) {
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

        // Mouse/touch drag for camera movement (direct, with momentum on release)
        // Skip mouse input if requested (e.g., when clicking on UI elements)
        if !skip_mouse_input {
            let mouse_pos = mouse_position();

            if is_mouse_button_pressed(MouseButton::Left) {
                // Start dragging
                self.is_dragging = true;
                self.last_mouse_x = mouse_pos.0;
                self.last_mouse_y = mouse_pos.1;
                self.last_drag_delta_x = 0.0;
                self.last_drag_delta_y = 0.0;
            }

            if is_mouse_button_down(MouseButton::Left) && self.is_dragging {
                // Calculate delta movement
                let delta_x = mouse_pos.0 - self.last_mouse_x;
                let delta_y = mouse_pos.1 - self.last_mouse_y;

                // Move camera directly in opposite direction (no velocity/interpolation)
                self.x -= delta_x;
                self.y -= delta_y;
                self.target_x = self.x;
                self.target_y = self.y;

                // Store delta for momentum
                self.last_drag_delta_x = delta_x;
                self.last_drag_delta_y = delta_y;

                // Update last position
                self.last_mouse_x = mouse_pos.0;
                self.last_mouse_y = mouse_pos.1;
            }

            if is_mouse_button_released(MouseButton::Left) && self.is_dragging {
                // Stop dragging and apply momentum
                self.is_dragging = false;

                // Apply momentum based on last drag delta
                self.target_x = self.x - self.last_drag_delta_x;
                self.target_y = self.y - self.last_drag_delta_y;
            }
        }

        // Trackpad/scroll wheel for camera movement (direct, with momentum)
        let scroll = mouse_wheel();
        if scroll.0 != 0.0 || scroll.1 != 0.0 {
            // Move camera directly with scroll (reversed for natural scrolling)
            // Scale the scroll values for appropriate speed
            let scroll_speed = 2.0;
            let scroll_delta_x = scroll.0 * scroll_speed;
            let scroll_delta_y = scroll.1 * scroll_speed;

            self.x -= scroll_delta_x;
            self.y -= scroll_delta_y;

            // Store scroll delta
            self.last_scroll_delta_x = scroll_delta_x;
            self.last_scroll_delta_y = scroll_delta_y;

            // Apply momentum immediately
            self.target_x = self.x - scroll_delta_x;
            self.target_y = self.y - scroll_delta_y;
        }
    }

    pub fn update(&mut self) {
        // Smoothly interpolate position and angle towards target
        self.x = lerp(self.x, self.target_x, self.lerp_factor);
        self.y = lerp(self.y, self.target_y, self.lerp_factor);
        self.angle = lerp(self.angle, self.target_angle, self.lerp_factor);
    }
}
