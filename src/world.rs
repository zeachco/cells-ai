use crate::camera::Camera;
use crate::cell::Cell;
use macroquad::prelude::*;

pub const WORLD_WIDTH: f32 = 5000.0;
pub const WORLD_HEIGHT: f32 = 5000.0;

pub struct World {
    pub cells: Vec<Cell>,
    pub camera: Camera,
}

impl World {
    pub fn spawn() -> Self {
        let mut cells = Vec::new();
        for _ in 0..2500 {
            cells.push(Cell::spawn());
        }

        World {
            cells,
            camera: Camera::new(),
        }
    }

    pub fn update(&mut self) {
        for cell in &mut self.cells {
            cell.update();
        }
        self.check_collisions();
    }

    pub fn check_collisions(&mut self) {
        for cell in &mut self.cells {
            // Wrap horizontally
            if cell.x < 0.0 {
                cell.x += WORLD_WIDTH;
            } else if cell.x > WORLD_WIDTH {
                cell.x -= WORLD_WIDTH;
            }

            // Wrap vertically
            if cell.y < 0.0 {
                cell.y += WORLD_HEIGHT;
            } else if cell.y > WORLD_HEIGHT {
                cell.y -= WORLD_HEIGHT;
            }
        }
    }

    pub fn render(&self) {
        // Render boundary lines
        self.render_boundaries();

        // Render cells
        for cell in &self.cells {
            cell.render(self.camera.x, self.camera.y);
        }
    }

    fn render_boundaries(&self) {
        let boundary_color = Color::new(0.3, 0.3, 0.3, 1.0);
        let line_thickness = 2.0;

        // Left boundary (x = 0)
        let left_screen_x = 0.0 - self.camera.x;
        draw_line(
            left_screen_x,
            0.0 - self.camera.y,
            left_screen_x,
            WORLD_HEIGHT - self.camera.y,
            line_thickness,
            boundary_color,
        );

        // Right boundary (x = WORLD_WIDTH)
        let right_screen_x = WORLD_WIDTH - self.camera.x;
        draw_line(
            right_screen_x,
            0.0 - self.camera.y,
            right_screen_x,
            WORLD_HEIGHT - self.camera.y,
            line_thickness,
            boundary_color,
        );

        // Top boundary (y = 0)
        let top_screen_y = 0.0 - self.camera.y;
        draw_line(
            0.0 - self.camera.x,
            top_screen_y,
            WORLD_WIDTH - self.camera.x,
            top_screen_y,
            line_thickness,
            boundary_color,
        );

        // Bottom boundary (y = WORLD_HEIGHT)
        let bottom_screen_y = WORLD_HEIGHT - self.camera.y;
        draw_line(
            0.0 - self.camera.x,
            bottom_screen_y,
            WORLD_WIDTH - self.camera.x,
            bottom_screen_y,
            line_thickness,
            boundary_color,
        );
    }
}
