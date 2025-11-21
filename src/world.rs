use crate::camera::Camera;
use crate::cell::Cell;
use macroquad::prelude::*;

pub const WORLD_WIDTH: f32 = 8000.0;
pub const WORLD_HEIGHT: f32 = 8000.0;

pub struct World {
    pub cells: Vec<Cell>,
    pub camera: Camera,
}

impl World {
    pub fn spawn() -> Self {
        let mut cells = Vec::new();
        for _ in 0..25000 {
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

        // Count stats
        let mut cells_in_viewport = 0;
        let screen_w = screen_width();
        let screen_h = screen_height();
        let radius = 10.0;
        let margin = radius * 1.5;

        // Render cells and count viewport cells
        for cell in &self.cells {
            // Check if cell is in viewport
            let screen_x = cell.x - self.camera.x;
            let screen_y = cell.y - self.camera.y;

            if !(screen_x < -margin
                || screen_x > screen_w + margin
                || screen_y < -margin
                || screen_y > screen_h + margin)
            {
                cells_in_viewport += 1;
            }

            cell.render(self.camera.x, self.camera.y);
        }

        // Render stats
        self.render_stats(cells_in_viewport);
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

    fn render_stats(&self, cells_in_viewport: usize) {
        // Count active cells (energy > 0)
        let active_cells = self.cells.iter().filter(|cell| cell.energy > 0.0).count();
        let total_cells = self.cells.len();

        // Render stats in top-left corner
        let padding = 20.0;
        let font_size = 24.0;
        let line_height = 30.0;
        let text_color = WHITE;

        // Line 1: Total active cells / total cells
        let line1 = format!("Active cells: {} / {}", active_cells, total_cells);
        draw_text(&line1, padding, padding + font_size, font_size, text_color);

        // Line 2: Cells in viewport
        let line2 = format!("Cells in viewport: {}", cells_in_viewport);
        draw_text(
            &line2,
            padding,
            padding + font_size + line_height,
            font_size,
            text_color,
        );
    }
}
