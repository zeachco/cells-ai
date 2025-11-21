use crate::camera::Camera;
use crate::cell::{Cell, CellState};
use macroquad::prelude::*;
use rayon::prelude::*;
use std::collections::VecDeque;

pub const WORLD_WIDTH: f32 = 8000.0;
pub const WORLD_HEIGHT: f32 = 8000.0;

// FPS performance targets
const TARGET_MIN_FPS: f32 = 30.0;
const TARGET_MAX_FPS: f32 = 240.0;
const FPS_SAMPLE_SIZE: usize = 60; // Track last 60 frames
const ADJUSTMENT_INTERVAL: f32 = 2.0; // Adjust cap every 2 seconds
const CELL_CAP_STEP: usize = 100; // Adjust cap by 100 cells at a time

// Read-only cell data for parallel collision detection
#[derive(Clone, Copy)]
struct CellCollisionData {
    x: f32,
    y: f32,
    radius: f32,
    energy_chunk_size: f32,
    species_multiplier: f32,
    state: CellState,
}

pub struct World {
    pub cells: Vec<Cell>,
    pub camera: Camera,
    max_cells: usize,
    frame_times: VecDeque<f32>,
    last_adjustment_time: f32,
    current_fps: f32,
}

impl World {
    pub fn spawn() -> Self {
        let mut cells = Vec::new();
        let initial_max_cells = 2500;
        for _ in 0..initial_max_cells {
            cells.push(Cell::spawn());
        }

        World {
            cells,
            camera: Camera::new(),
            max_cells: initial_max_cells,
            frame_times: VecDeque::with_capacity(FPS_SAMPLE_SIZE),
            last_adjustment_time: 0.0,
            current_fps: 60.0, // Initial estimate
        }
    }

    pub fn update(&mut self, delta_time: f32) {
        // Update FPS tracking
        self.update_fps(delta_time);

        // Adjust max_cells cap based on FPS
        self.adjust_cell_cap();

        // Parallel cell updates
        self.cells.par_iter_mut().for_each(|cell| {
            cell.update();
        });

        self.check_collisions();
        self.handle_reproduction();
    }

    fn update_fps(&mut self, delta_time: f32) {
        // Add current frame time
        self.frame_times.push_back(delta_time);

        // Keep only the last FPS_SAMPLE_SIZE frames
        while self.frame_times.len() > FPS_SAMPLE_SIZE {
            self.frame_times.pop_front();
        }

        // Calculate average FPS
        if !self.frame_times.is_empty() {
            let avg_frame_time: f32 = self.frame_times.iter().sum::<f32>() / self.frame_times.len() as f32;
            self.current_fps = if avg_frame_time > 0.0 {
                1.0 / avg_frame_time
            } else {
                60.0 // Fallback
            };
        }
    }

    fn adjust_cell_cap(&mut self) {
        // Only adjust every ADJUSTMENT_INTERVAL seconds
        self.last_adjustment_time += self.frame_times.back().unwrap_or(&0.016);

        if self.last_adjustment_time < ADJUSTMENT_INTERVAL {
            return;
        }

        self.last_adjustment_time = 0.0;

        // Adjust cap based on FPS
        if self.current_fps < TARGET_MIN_FPS {
            // FPS too low, reduce cap
            if self.max_cells > CELL_CAP_STEP {
                self.max_cells = self.max_cells.saturating_sub(CELL_CAP_STEP);
            }
        } else if self.current_fps > TARGET_MAX_FPS {
            // FPS too high, increase cap
            self.max_cells += CELL_CAP_STEP;
        }
        // If FPS is between 30-240, don't change the cap
    }

    fn handle_reproduction(&mut self) {
        let mut new_cells = Vec::new();
        let current_cell_count = self.cells.len();

        for cell in &mut self.cells {
            if cell.energy > 100.0 {
                // Check if we're at or over the max_cells cap
                let projected_count = current_cell_count + new_cells.len();
                if projected_count >= self.max_cells {
                    // Cap reached: cell keeps its energy and cannot reproduce
                    continue;
                }

                // Calculate energy distribution: 2/3 to child, 1/3 to parent
                let total_energy = cell.energy;
                let child_energy = (total_energy * 2.0) / 3.0;
                let parent_energy = total_energy / 3.0;

                // Create child cell
                let mut child = cell.spawn_child();
                child.energy = child_energy;
                new_cells.push(child);

                // Update parent energy
                cell.energy = parent_energy;
            }
        }

        // Add new cells to the world
        self.cells.extend(new_cells);
    }

    pub fn check_collisions(&mut self) {
        // Extract read-only collision data for parallel processing
        let collision_data: Vec<CellCollisionData> = self
            .cells
            .iter()
            .map(|cell| CellCollisionData {
                x: cell.x,
                y: cell.y,
                radius: cell.get_current_radius(), // Use age-based radius
                energy_chunk_size: cell.energy_chunk_size,
                species_multiplier: cell.species_multiplier,
                state: cell.state,
            })
            .collect();

        // Parallel collision detection - returns (alive_cell_index, corpse_cell_index, chunk_size, multiplier)
        let collisions: Vec<(usize, usize, f32, f32)> = (0..collision_data.len())
            .into_par_iter()
            .filter_map(|i| {
                // Skip corpse cells as energy donors
                if collision_data[i].state == CellState::Corpse {
                    return None;
                }

                // Check for collision with any corpse cell
                for j in 0..collision_data.len() {
                    if i == j || collision_data[j].state == CellState::Alive {
                        continue; // Skip self and alive cells
                    }

                    let dx = collision_data[i].x - collision_data[j].x;
                    let dy = collision_data[i].y - collision_data[j].y;
                    let distance_squared = dx * dx + dy * dy;
                    let collision_distance = collision_data[i].radius + collision_data[j].radius;
                    let collision_distance_squared = collision_distance * collision_distance;

                    if distance_squared < collision_distance_squared {
                        // Return (alive_idx, corpse_idx, chunk_size, species_multiplier)
                        return Some((
                            i,
                            j,
                            collision_data[i].energy_chunk_size,
                            collision_data[i].species_multiplier,
                        ));
                    }
                }

                None
            })
            .collect();

        // Apply energy transfers to alive cells and reduce energy from corpse cells
        for (alive_idx, corpse_idx, chunk_size, multiplier) in &collisions {
            // Alive cell gains energy through gain_energy() (handles growth mechanic)
            self.cells[*alive_idx].gain_energy(chunk_size * multiplier);
            // Corpse cell loses base chunk_size
            self.cells[*corpse_idx].energy -= chunk_size;
        }

        // Collect cells with energy < -100 to remove
        let mut indices_to_remove: Vec<usize> = self
            .cells
            .iter()
            .enumerate()
            .filter_map(|(idx, cell)| {
                if cell.energy < -100.0 {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect();

        // Remove depleted cells in reverse order to maintain index validity
        // Using unstable sort for performance we don't need to order them, we just mutate directly
        // to clean up
        indices_to_remove.sort_unstable();
        for &idx in indices_to_remove.iter().rev() {
            self.cells.swap_remove(idx);
        }

        // Parallel boundary wrapping
        self.cells.par_iter_mut().for_each(|cell| {
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
        });
    }

    pub fn render(&self) {
        // Render boundary lines
        self.render_boundaries();

        // Count stats
        let mut cells_in_viewport = 0;
        let screen_w = screen_width();
        let screen_h = screen_height();

        // Render cells and count viewport cells
        for cell in &self.cells {
            // Check if cell is in viewport
            let screen_x = cell.x - self.camera.x;
            let screen_y = cell.y - self.camera.y;
            let margin = cell.get_current_radius() * 1.5;

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
        // Count active cells (state == Alive)
        let active_cells = self
            .cells
            .iter()
            .filter(|cell| cell.state == CellState::Alive)
            .count();
        let total_cells = self.cells.len();

        // Render stats in top-left corner
        let padding = 20.0;
        let font_size = 24.0;
        let line_height = 30.0;
        let text_color = WHITE;

        // Line 1: FPS
        let line1 = format!("FPS: {:.1}", self.current_fps);
        draw_text(&line1, padding, padding + font_size, font_size, text_color);

        // Line 2: Total active cells / total cells / max cap
        let line2 = format!(
            "Cells: {} / {} (cap: {})",
            active_cells, total_cells, self.max_cells
        );
        draw_text(
            &line2,
            padding,
            padding + font_size + line_height,
            font_size,
            text_color,
        );

        // Line 3: Cells in viewport
        let line3 = format!("Viewport: {}", cells_in_viewport);
        draw_text(
            &line3,
            padding,
            padding + font_size + line_height * 2.0,
            font_size,
            text_color,
        );
    }
}
