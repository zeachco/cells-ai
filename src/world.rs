use crate::camera::Camera;
use crate::cell::{Cell, CellState};
use crate::config::{SimulationConfig, get_config};
use crate::spatial_grid::SpatialGrid;
use crate::stats::Stats;
use macroquad::prelude::*;
use rayon::prelude::*;
use std::collections::VecDeque;

// FPS performance targets
const TARGET_MIN_FPS: f32 = 30.0;
const TARGET_MAX_FPS: f32 = 240.0;
const FPS_SAMPLE_SIZE: usize = 60; // Track last 60 frames
const ADJUSTMENT_INTERVAL: f32 = 2.0; // Adjust cap every 2 seconds
const CELL_CAP_STEP: usize = 100; // Adjust cap by 100 cells at a time

// World simulation constants
pub const SENSOR_RANGE: f32 = 200.0; // Public so cells can normalize sensor inputs
const SENSOR_COUNT: usize = 5;
const REPRODUCTION_ENERGY_THRESHOLD: f32 = 100.0;
const CHILD_ENERGY_RATIO: f32 = 2.0 / 3.0;
const PARENT_ENERGY_RATIO: f32 = 1.0 / 3.0;
const DEPLETED_CELL_ENERGY: f32 = -100.0;

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
    spatial_grid: SpatialGrid,
    max_cells: usize,
    frame_times: VecDeque<f32>,
    last_adjustment_time: f32,
    current_fps: f32,
    pub stats: Stats,
    best_cell_genome: Option<Cell>, // Store the complete best cell for respawning
    last_best_cell_index: Option<usize>, // Track last best cell to avoid redundant clones
    selected_cell_index: Option<usize>, // Currently selected cell for highlighting
    // Simulation controls
    pub paused: bool,
    pub simulation_speed: f32, // 1.0 = normal, 0.5 = half speed, 2.0 = double speed
    // Diversity tracking
    pub color_diversity: f32, // 0.0 = no diversity, 1.0 = maximum diversity
    // Configuration
    config: SimulationConfig,
}

impl World {
    pub fn spawn() -> Self {
        let config = get_config();
        let mut cells = Vec::new();
        for _ in 0..config.initial_cell_count {
            cells.push(Cell::spawn(config.world_width, config.world_height));
        }

        World {
            cells,
            camera: Camera::new(),
            spatial_grid: SpatialGrid::new(config.world_width, config.world_height, 100.0),
            max_cells: config.initial_cell_count,
            frame_times: VecDeque::with_capacity(FPS_SAMPLE_SIZE),
            last_adjustment_time: 0.0,
            current_fps: 60.0, // Initial estimate
            stats: Stats::new(),
            best_cell_genome: None,
            last_best_cell_index: None,
            selected_cell_index: None,
            paused: false,
            simulation_speed: 1.0,
            color_diversity: 0.0,
            config,
        }
    }

    // Reset the world with spawns from the best cell's genome
    pub fn respawn_from_best(&mut self) {
        if let Some(best_cell) = &self.best_cell_genome {
            // Clear current cells
            self.cells.clear();

            // Spawn new cells based on the best cell's genome
            let spawn_count = self.max_cells.min(self.config.initial_cell_count);
            for _ in 0..spawn_count {
                let mut new_cell = best_cell.spawn_child();

                // Randomize position across the entire map
                new_cell.x = rand::gen_range(0.0, self.config.world_width);
                new_cell.y = rand::gen_range(0.0, self.config.world_height);

                // Give them starting energy
                new_cell.energy = 100.0;

                self.cells.push(new_cell);
            }

            println!(
                "World reset! Spawned {} cells from best genome (fitness: {:.1})",
                spawn_count,
                best_cell.total_energy_accumulated + (best_cell.children_count as f32 * 100.0)
            );
        }
    }

    pub fn update(&mut self, delta_time: f32) {
        // Handle keyboard controls
        self.handle_keyboard_input();

        // Update FPS tracking
        self.update_fps(delta_time);

        // Skip simulation if paused
        if self.paused {
            return;
        }

        // Note: simulation_speed affects how many updates we process
        // For simplicity, we just run more/fewer frames naturally with FPS changes

        // Adjust max_cells cap based on FPS
        self.adjust_cell_cap();

        // Capture world dimensions for parallel context
        let world_width = self.config.world_width;
        let world_height = self.config.world_height;

        // Parallel cell updates (speed affects energy costs and movement)
        // Note: For simplicity, we update normally and accept speed affects FPS
        self.cells.par_iter_mut().for_each(|cell| {
            cell.update(world_width, world_height);
        });

        // Build spatial grid for collision detection
        self.rebuild_spatial_grid();
        self.check_collisions();

        self.handle_reproduction();

        // Rebuild spatial grid after collisions/reproduction changed cell array
        self.rebuild_spatial_grid();
        self.update_sensors();

        self.update_stats();

        // Check for extinction and respawn if needed (after stats to ensure best_cell_genome is set)
        // Count alive cells
        let alive_count = self
            .cells
            .iter()
            .filter(|c| c.state == CellState::Alive)
            .count();
        if alive_count == 0 && self.best_cell_genome.is_some() {
            self.respawn_from_best();
        }

        // Update camera to follow selected cell if stats box is selected
        if let Some((x, y)) = self.stats.get_selected_position() {
            // Center the camera on the selected cell using 10% of the delta
            let screen_w = screen_width();
            let screen_h = screen_height();
            let target_camera_x = x - screen_w / 2.0;
            let target_camera_y = y - screen_h / 2.0;

            // Calculate delta (distance to target)
            let delta_x = target_camera_x - self.camera.x;
            let delta_y = target_camera_y - self.camera.y;

            // Move camera by 10% of the delta
            self.camera.target_x = self.camera.x + delta_x * 0.1;
            self.camera.target_y = self.camera.y + delta_y * 0.1;
        }
    }

    // Handle mouse clicks on the stats box
    pub fn handle_stats_click(&mut self) {
        if is_mouse_button_pressed(MouseButton::Left) {
            let mouse_pos = mouse_position();
            if self.stats.is_mouse_over(mouse_pos.0, mouse_pos.1) {
                self.stats.toggle_selection();
            }
        }
    }

    // Handle keyboard input for simulation controls
    fn handle_keyboard_input(&mut self) {
        // Space: Toggle pause
        if is_key_pressed(KeyCode::Space) {
            self.paused = !self.paused;
            println!(
                "Simulation {}",
                if self.paused { "PAUSED" } else { "RESUMED" }
            );
        }

        // R: Manual reset with best genome
        if is_key_pressed(KeyCode::R) {
            if self.best_cell_genome.is_some() {
                self.respawn_from_best();
                println!("Manual reset triggered");
            } else {
                println!("No best genome available for reset");
            }
        }

        // + or =: Increase speed
        if is_key_pressed(KeyCode::Equal) || is_key_pressed(KeyCode::KpAdd) {
            self.simulation_speed = (self.simulation_speed * 1.5).min(8.0);
            println!("Simulation speed: {:.1}x", self.simulation_speed);
        }

        // - or _: Decrease speed
        if is_key_pressed(KeyCode::Minus) || is_key_pressed(KeyCode::KpSubtract) {
            self.simulation_speed = (self.simulation_speed / 1.5).max(0.125);
            println!("Simulation speed: {:.1}x", self.simulation_speed);
        }

        // 1: Reset to normal speed
        if is_key_pressed(KeyCode::Key1) {
            self.simulation_speed = 1.0;
            println!("Simulation speed: 1.0x (normal)");
        }
    }

    // Rebuild spatial grid with all current cell positions
    fn rebuild_spatial_grid(&mut self) {
        self.spatial_grid.clear();
        for (idx, cell) in self.cells.iter().enumerate() {
            self.spatial_grid.insert(cell.x, cell.y, idx);
        }
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
            let avg_frame_time: f32 =
                self.frame_times.iter().sum::<f32>() / self.frame_times.len() as f32;
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
        let best_cell_idx = self.last_best_cell_index;

        for (idx, cell) in self.cells.iter_mut().enumerate() {
            if cell.energy > REPRODUCTION_ENERGY_THRESHOLD {
                // Check if we're at or over the max_cells cap
                let projected_count = current_cell_count + new_cells.len();
                if projected_count >= self.max_cells {
                    // Cap reached: cell keeps its energy and cannot reproduce
                    continue;
                }

                // Calculate energy distribution
                let total_energy = cell.energy;
                let child_energy = total_energy * CHILD_ENERGY_RATIO;
                let parent_energy = total_energy * PARENT_ENERGY_RATIO;

                // Create child cell
                let mut child = cell.spawn_child();
                child.energy = child_energy;
                new_cells.push(child);

                // Update parent energy and increment children count
                cell.energy = parent_energy;
                cell.children_count += 1;

                // Save neural network if this is the best cell reproducing
                if Some(idx) == best_cell_idx {
                    crate::storage::save_best_neural_network(&cell.brain, cell.generation);
                }
            }
        }

        // Add new cells to the world
        self.cells.extend(new_cells);
    }

    fn update_sensors(&mut self) {
        // Spatial grid already built in update(), reuse it
        // Extract cell data for sensor calculations
        let cell_data: Vec<(f32, f32, f32, f32, f32)> = self
            .cells
            .iter()
            .map(|c| {
                let is_alive = if c.state == CellState::Alive {
                    1.0
                } else {
                    0.0
                };
                (c.x, c.y, c.energy, c.mass, is_alive)
            })
            .collect();

        // Capture world dimensions for parallel context
        let world_width = self.config.world_width;
        let world_height = self.config.world_height;

        // Update sensors for each cell in parallel
        self.cells.par_iter_mut().enumerate().for_each(|(i, cell)| {
            // Query nearby cells using spatial grid
            let nearby_indices = self.spatial_grid.query_nearby(cell.x, cell.y, SENSOR_RANGE);

            // Calculate distances and angles to all nearby cells
            let mut sensor_data: Vec<(usize, f32, f32, f32, f32)> = nearby_indices
                .iter()
                .filter_map(|&j| {
                    if i == j {
                        return None; // Skip self
                    }

                    // Bounds check for safety
                    if j >= cell_data.len() {
                        return None;
                    }

                    let (x2, y2, _energy, mass, is_alive) = cell_data[j];

                    // Handle wrapping distance calculation
                    let mut dx = x2 - cell.x;
                    let mut dy = y2 - cell.y;

                    // Adjust for world wrapping
                    if dx.abs() > world_width / 2.0 {
                        dx = dx - dx.signum() * world_width;
                    }
                    if dy.abs() > world_height / 2.0 {
                        dy = dy - dy.signum() * world_height;
                    }

                    let distance = (dx * dx + dy * dy).sqrt();

                    // Filter out cells that are too far
                    if distance > SENSOR_RANGE {
                        return None;
                    }

                    // Calculate angle to target relative to cell's facing direction
                    let angle_to_target = dy.atan2(dx);
                    let mut angle_from_front = angle_to_target - cell.angle;

                    // Normalize angle to -PI..PI range
                    while angle_from_front > std::f32::consts::PI {
                        angle_from_front -= std::f32::consts::TAU;
                    }
                    while angle_from_front < -std::f32::consts::PI {
                        angle_from_front += std::f32::consts::TAU;
                    }

                    // Return (index, angle_from_front, distance, mass, is_alive)
                    Some((j, angle_from_front, distance, mass, is_alive))
                })
                .collect();

            // Use partial sort to get top SENSOR_COUNT without sorting the entire vec
            // Priority: dead > alive, then high energy > low energy, then close > far
            if sensor_data.len() > SENSOR_COUNT {
                // Use select_nth_unstable to partition around the (SENSOR_COUNT-1)th element
                // This partitions so elements [0..SENSOR_COUNT] are the smallest/best
                sensor_data.select_nth_unstable_by(SENSOR_COUNT - 1, |a, b| {
                    let is_alive_a = a.4; // 1.0 if alive, 0.0 if dead
                    let is_alive_b = b.4;
                    let energy_a = cell_data.get(a.0).map(|d| d.2).unwrap_or(0.0);
                    let energy_b = cell_data.get(b.0).map(|d| d.2).unwrap_or(0.0);

                    // Sort by alive status ascending (0.0 before 1.0, so dead before alive)
                    // Then by energy descending, then by distance ascending
                    is_alive_a
                        .partial_cmp(&is_alive_b)
                        .unwrap()
                        .then(energy_b.partial_cmp(&energy_a).unwrap())
                        .then(a.2.partial_cmp(&b.2).unwrap())
                });
                // Keep only the top SENSOR_COUNT
                sensor_data.truncate(SENSOR_COUNT);
            }

            cell.nearest_cells = sensor_data;
        });
    }

    fn update_stats(&mut self) {
        // First, find the cell with the highest fitness (only alive cells)
        let mut best_fitness = f32::MIN;
        let mut best_cell_index = None;
        let mut alive_cells = Vec::new();

        for (i, cell) in self.cells.iter().enumerate() {
            // Only consider alive cells
            if cell.state == CellState::Alive {
                alive_cells.push(cell);

                // Use same fitness calculation as Stats
                let fitness = cell.total_energy_accumulated + (cell.children_count as f32 * 100.0);
                if fitness > best_fitness {
                    best_fitness = fitness;
                    best_cell_index = Some(i);
                }
            }
        }

        // Calculate color diversity (hue variance)
        if alive_cells.len() > 1 {
            let hues: Vec<f32> = alive_cells
                .iter()
                .map(|cell| {
                    let (h, _, _) = Cell::rgb_to_hsv_public(cell.color);
                    h
                })
                .collect();

            // Calculate variance of hues
            let mean_hue: f32 = hues.iter().sum::<f32>() / hues.len() as f32;
            let variance: f32 = hues
                .iter()
                .map(|h| {
                    let diff = (h - mean_hue).abs();
                    // Handle wraparound (hue is 0-360)
                    let diff = if diff > 180.0 { 360.0 - diff } else { diff };
                    diff * diff
                })
                .sum::<f32>()
                / hues.len() as f32;

            // Normalize diversity to 0-1 (max variance is ~180^2)
            self.color_diversity = (variance / (180.0 * 180.0)).min(1.0);
        } else {
            self.color_diversity = 0.0;
        }

        // Update stats and genome with the best cell only, or clear if no alive cells
        if let Some(index) = best_cell_index {
            let best_cell = &self.cells[index];

            // Only clone the best cell's genome if it changed (avoid expensive clone every frame)
            if self.last_best_cell_index != Some(index) {
                self.best_cell_genome = Some(best_cell.clone());
                self.last_best_cell_index = Some(index);
            }

            // Set stats to show the current best alive cell
            self.stats.set(crate::stats::BestCellStats {
                total_energy_accumulated: best_cell.total_energy_accumulated,
                current_energy: best_cell.energy,
                children_count: best_cell.children_count,
                generation: best_cell.generation,
                color: best_cell.color,
                age: best_cell.age,
                x: best_cell.x,
                y: best_cell.y,
                is_alive: true, // is_alive is guaranteed true since we filtered for it
            });

            // Update selected cell index if stats are selected
            if self.stats.is_selected() {
                self.selected_cell_index = Some(index);
            } else {
                self.selected_cell_index = None;
            }
        } else {
            // No alive cells found, clear the stats
            self.stats.clear();
            self.last_best_cell_index = None;
            self.selected_cell_index = None;
        }
    }

    pub fn check_collisions(&mut self) {
        // Spatial grid already built in update(), reuse it
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

        // Capture world dimensions for parallel context
        let world_width = self.config.world_width;
        let world_height = self.config.world_height;

        // Parallel collision detection using spatial grid
        // Returns (alive_cell_index, corpse_cell_index, chunk_size, multiplier)
        let collisions: Vec<(usize, usize, f32, f32)> = (0..collision_data.len())
            .into_par_iter()
            .filter_map(|i| {
                // Skip corpse cells as energy donors
                if collision_data[i].state == CellState::Corpse {
                    return None;
                }

                let cell_i = &collision_data[i];

                // Query nearby cells using spatial grid instead of checking all cells
                let nearby_indices = self.spatial_grid.query_nearby(
                    cell_i.x,
                    cell_i.y,
                    cell_i.radius * 3.0, // Query radius (conservative estimate)
                );

                // Check for collision with nearby corpse cells only
                for &j in &nearby_indices {
                    if i == j || collision_data[j].state == CellState::Alive {
                        continue; // Skip self and alive cells
                    }

                    let cell_j = &collision_data[j];

                    // Handle wrapping distance calculation
                    let mut dx = cell_i.x - cell_j.x;
                    let mut dy = cell_i.y - cell_j.y;

                    // Adjust for world wrapping
                    if dx.abs() > world_width / 2.0 {
                        dx = dx - dx.signum() * world_width;
                    }
                    if dy.abs() > world_height / 2.0 {
                        dy = dy - dy.signum() * world_height;
                    }

                    let distance_squared = dx * dx + dy * dy;
                    let collision_distance = cell_i.radius + cell_j.radius;
                    let collision_distance_squared = collision_distance * collision_distance;

                    if distance_squared < collision_distance_squared {
                        // Return (alive_idx, corpse_idx, chunk_size, species_multiplier)
                        return Some((i, j, cell_i.energy_chunk_size, cell_i.species_multiplier));
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

        // Collect cells with energy below threshold to remove
        let mut indices_to_remove: Vec<usize> = self
            .cells
            .iter()
            .enumerate()
            .filter_map(|(idx, cell)| {
                if cell.energy < DEPLETED_CELL_ENERGY {
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

        // Boundary wrapping now handled inline in cell.update()
    }

    fn render_sensor_lines(&self) {
        for cell in &self.cells {
            // Only draw sensors for alive cells
            if cell.state != CellState::Alive {
                continue;
            }

            for &(target_idx, _angle, _distance, _mass, _is_alive) in &cell.nearest_cells {
                // Safety check: ensure target index is valid
                if target_idx >= self.cells.len() {
                    continue;
                }

                let target = &self.cells[target_idx];

                // Calculate vector from cell to target
                let mut dx = target.x - cell.x;
                let mut dy = target.y - cell.y;

                // Handle world wrapping for line drawing
                if dx.abs() > self.config.world_width / 2.0 {
                    dx = dx - dx.signum() * self.config.world_width;
                }
                if dy.abs() > self.config.world_height / 2.0 {
                    dy = dy - dy.signum() * self.config.world_height;
                }

                // Calculate target position accounting for wrapping
                let target_x = cell.x + dx;
                let target_y = cell.y + dy;

                // Calculate angle to target
                let angle_to_target = dy.atan2(dx);

                // Normalize angles to -PI to PI range
                let mut angle_diff = angle_to_target - cell.angle;
                while angle_diff > std::f32::consts::PI {
                    angle_diff -= std::f32::consts::TAU;
                }
                while angle_diff < -std::f32::consts::PI {
                    angle_diff += std::f32::consts::TAU;
                }

                // Calculate opacity based on how much the target is in front
                // 0 degrees (directly in front) = full opacity
                // ±180 degrees (directly behind) = no opacity
                let normalized_angle = angle_diff.abs() / std::f32::consts::PI; // 0.0 to 1.0
                let opacity = (1.0 - normalized_angle).max(0.0);

                // Convert to screen coordinates
                let x1 = cell.x - self.camera.x;
                let y1 = cell.y - self.camera.y;
                let x2 = target_x - self.camera.x;
                let y2 = target_y - self.camera.y;

                // Create color with opacity (use cell's color)
                let line_color = Color::new(
                    cell.color.r,
                    cell.color.g,
                    cell.color.b,
                    opacity * 0.3, // Scale down opacity for subtlety
                );

                // Draw line
                draw_line(x1, y1, x2, y2, 1.0, line_color);
            }
        }
    }

    pub fn render(&self) {
        // Render boundary lines (only if UI enabled)
        if self.config.show_ui {
            self.render_boundaries();

            // Render sensor lines first (so they appear behind cells)
            self.render_sensor_lines();
        }

        // Count stats
        let mut cells_in_viewport = 0;
        let screen_w = screen_width();
        let screen_h = screen_height();

        // Render cells and count viewport cells
        for (idx, cell) in self.cells.iter().enumerate() {
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

            // Render the cell
            cell.render(self.camera.x, self.camera.y);

            // Draw gold stroke around selected cell (only if UI enabled)
            if self.config.show_ui && self.selected_cell_index == Some(idx) {
                let current_radius = cell.get_current_radius();
                let gold = Color::new(1.0, 0.84, 0.0, 1.0); // Gold color
                draw_circle_lines(
                    screen_x,
                    screen_y,
                    current_radius + 3.0, // Slightly larger than cell
                    4.0,                  // Thickness
                    gold,
                );
            }
        }

        // Render stats (only if UI enabled)
        if self.config.show_ui {
            self.render_stats(cells_in_viewport);

            // Render best cell stats (top-right corner)
            self.stats.render();
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
            self.config.world_height - self.camera.y,
            line_thickness,
            boundary_color,
        );

        // Right boundary (x = world_width)
        let right_screen_x = self.config.world_width - self.camera.x;
        draw_line(
            right_screen_x,
            0.0 - self.camera.y,
            right_screen_x,
            self.config.world_height - self.camera.y,
            line_thickness,
            boundary_color,
        );

        // Top boundary (y = 0)
        let top_screen_y = 0.0 - self.camera.y;
        draw_line(
            0.0 - self.camera.x,
            top_screen_y,
            self.config.world_width - self.camera.x,
            top_screen_y,
            line_thickness,
            boundary_color,
        );

        // Bottom boundary (y = world_height)
        let bottom_screen_y = self.config.world_height - self.camera.y;
        draw_line(
            0.0 - self.camera.x,
            bottom_screen_y,
            self.config.world_width - self.camera.x,
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

        // Line 4: Simulation state (paused/speed)
        let state_text = if self.paused {
            format!("PAUSED (Speed: {:.1}x)", self.simulation_speed)
        } else {
            format!("Speed: {:.1}x", self.simulation_speed)
        };
        let state_color = if self.paused { YELLOW } else { WHITE };
        draw_text(
            &state_text,
            padding,
            padding + font_size + line_height * 3.0,
            font_size,
            state_color,
        );

        // Line 5: Diversity metric
        let diversity_text = format!("Diversity: {:.1}%", self.color_diversity * 100.0);
        let diversity_color = if self.color_diversity < 0.2 {
            Color::new(1.0, 0.5, 0.5, 1.0) // Low diversity warning (red-ish)
        } else {
            Color::new(0.5, 1.0, 0.5, 1.0) // Good diversity (green-ish)
        };
        draw_text(
            &diversity_text,
            padding,
            padding + font_size + line_height * 4.0,
            font_size,
            diversity_color,
        );

        // Controls help (bottom-left)
        let help_y = screen_height() - padding - font_size * 3.0;
        let help_font_size = 18.0;
        let help_color = Color::new(0.7, 0.7, 0.7, 1.0);
        draw_text(
            "Controls: SPACE=Pause | R=Reset | +/-=Speed | 1=Normal Speed",
            padding,
            help_y,
            help_font_size,
            help_color,
        );
    }
}
