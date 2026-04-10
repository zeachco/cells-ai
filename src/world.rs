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
const TARGET_GOOD_FPS: f32 = 60.0;
const TARGET_MAX_FPS: f32 = 240.0;
const FPS_SAMPLE_SIZE: usize = 60; // Track last 60 frames
const ADJUSTMENT_INTERVAL: f32 = 2.0; // Adjust cap every 2 seconds
const CELL_CAP_STEP: usize = 100; // Adjust cap by 100 cells at a time
const CELL_CAP_SLOW_STEP: usize = 20; // Slow increase when FPS is good but not maxed

// World simulation constants
pub const SENSOR_RANGE: f32 = 200.0; // Public so cells can normalize sensor inputs
const SENSOR_RANGE_SQUARED: f32 = SENSOR_RANGE * SENSOR_RANGE;
const SENSOR_COUNT: usize = 5;
pub const REPRODUCTION_ENERGY_THRESHOLD: f32 = 100.0; // Public for energy normalization
const CHILD_ENERGY_RATIO: f32 = 2.0 / 3.0;
const PARENT_ENERGY_RATIO: f32 = 1.0 / 3.0;
pub const DEPLETED_CELL_ENERGY: f32 = -100.0; // Public for energy normalization

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
    followed_cell_death_time: Option<f64>, // Track when the followed cell died
    // Simulation controls
    pub paused: bool,
    pub simulation_speed: f32, // 1.0 = normal, 0.5 = half speed, 2.0 = double speed
    // Diversity tracking
    pub color_diversity: f32, // 0.0 = no diversity, 1.0 = maximum diversity
    pub tier_cell_counts: [usize; 4],
    pub tier_diversities: [f32; 4],
    // Configuration
    config: SimulationConfig,
    // Cached best neural networks per tier (brain, generation) - loaded once from storage
    cached_best_brains: [Option<(crate::neural_network::NeuralNetwork, usize)>; 4],
    // Best saved score per tier (to avoid saving worse models)
    best_saved_scores: [f32; 4],
    // Custom font for rendering
    font: Option<Font>,
    // Parallax star-field background
    background: Option<crate::background::Background>,
}

impl World {
    pub fn spawn(font: Option<Font>) -> Self {
        let config = get_config();

        // Load best brain for each tier from storage
        let mut cached_best_brains: [Option<(crate::neural_network::NeuralNetwork, usize)>; 4] =
            [None, None, None, None];
        let mut best_saved_scores = [0.0f32; 4];

        for tier in 0..4 {
            let loaded_data = crate::storage::load_best_neural_network(tier);
            if let Some((brain, generation, score)) = loaded_data {
                cached_best_brains[tier] = Some((brain, generation));
                best_saved_scores[tier] = score;
            }
        }

        let mut cells = Vec::new();
        for i in 0..config.initial_cell_count {
            let tier = i % 4;
            let mut cell = Cell::spawn(
                config.world_width,
                config.world_height,
                tier,
                &cached_best_brains[tier],
            );
            // Half the population starts with low energy so they die quickly,
            // seeding the world with corpses for others to eat.
            if i % 2 == 1 {
                cell.energy = rand::gen_range(0.0, REPRODUCTION_ENERGY_THRESHOLD * 0.5);
            }
            cells.push(cell);
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
            followed_cell_death_time: None,
            paused: false,
            simulation_speed: 1.0,
            color_diversity: 0.0,
            tier_cell_counts: [0; 4],
            tier_diversities: [0.0; 4],
            config,
            cached_best_brains,
            best_saved_scores,
            font,
            background: match crate::background::Background::new() {
                Ok(bg) => Some(bg),
                Err(e) => {
                    eprintln!("Background shader failed to load: {e:?}");
                    None
                }
            },
        }
    }

    // Reset the world with spawns from the best cell's genome
    pub fn respawn_from_best(&mut self) {
        // Clear current cells
        self.cells.clear();

        // Spawn new cells equally distributed across all 4 tiers
        let spawn_count = self.max_cells.min(self.config.initial_cell_count);

        // Calculate cells per tier (evenly distributed)
        let cells_per_tier = spawn_count / 4;
        let remainder = spawn_count % 4;

        for tier in 0..4 {
            // Distribute remainder across first N tiers
            let tier_count = cells_per_tier + if tier < remainder { 1 } else { 0 };

            for _ in 0..tier_count {
                let mut new_cell = Cell::spawn(
                    self.config.world_width,
                    self.config.world_height,
                    tier,
                    &self.cached_best_brains[tier],
                );

                // Give them starting energy
                new_cell.energy = 100.0;

                self.cells.push(new_cell);
            }
        }

        println!(
            "World reset! Spawned {} cells ({} per tier)",
            spawn_count, cells_per_tier
        );
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

        // Save best cell's brain if it just died and score improved
        if let Some(best_idx) = self.last_best_cell_index
            && best_idx < self.cells.len()
        {
            let best_cell = &self.cells[best_idx];
            if best_cell.state == CellState::Corpse {
                let score = best_cell.score();
                let tier = best_cell.brain_tier;
                // Only save if score is positive and better than previous best for this tier
                if score > self.best_saved_scores[tier] {
                    let brain_clone = best_cell.brain.clone();
                    let generation = best_cell.generation;
                    let children = best_cell.children_count;
                    let energy = best_cell.energy_from_cells;
                    let age = best_cell.age;
                    crate::storage::save_best_neural_network(
                        tier,
                        &brain_clone,
                        generation,
                        score,
                        children,
                        energy,
                        age,
                    );
                    // Update cache and best score
                    self.cached_best_brains[tier] = Some((brain_clone, generation));
                    self.best_saved_scores[tier] = score;
                    println!("📈 New high score (tier {}): {:.1}", tier, score);
                }
            }
        }

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
            let mut delta_x = target_camera_x - self.camera.x;
            let mut delta_y = target_camera_y - self.camera.y;

            // Account for world wrapping - take shortest path around toroidal world
            let world_width = self.config.world_width;
            let world_height = self.config.world_height;

            if delta_x.abs() > world_width / 2.0 {
                if delta_x > 0.0 {
                    delta_x -= world_width;
                } else {
                    delta_x += world_width;
                }
            }

            if delta_y.abs() > world_height / 2.0 {
                if delta_y > 0.0 {
                    delta_y -= world_height;
                } else {
                    delta_y += world_height;
                }
            }

            // Move camera towards target using configured tracking speed
            self.camera.target_x = self.camera.x + delta_x * self.config.camera_tracking_speed;
            self.camera.target_y = self.camera.y + delta_y * self.config.camera_tracking_speed;
        }
    }

    // Check if mouse is over stats box
    pub fn is_mouse_over_stats(&self, mouse_x: f32, mouse_y: f32) -> bool {
        self.stats
            .is_mouse_over(mouse_x, mouse_y, self.font.as_ref())
    }

    // Handle mouse clicks on the stats box
    pub fn handle_stats_click(&mut self) {
        if is_mouse_button_pressed(MouseButton::Left) {
            let mouse_pos = mouse_position();
            if self
                .stats
                .is_mouse_over(mouse_pos.0, mouse_pos.1, self.font.as_ref())
            {
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
            self.max_cells = self.max_cells.saturating_sub(CELL_CAP_STEP);
        } else if self.current_fps > TARGET_MAX_FPS {
            // FPS very high, increase cap quickly
            self.max_cells += CELL_CAP_STEP;
        } else if self.current_fps > TARGET_GOOD_FPS {
            // FPS comfortably above 60, slowly grow the cap
            self.max_cells += CELL_CAP_SLOW_STEP;
        }
        // If FPS is between 30-60, don't change the cap
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

                // Save neural network if this is the best cell reproducing AND score improved
                if Some(idx) == best_cell_idx {
                    let score = cell.score();
                    let tier = cell.brain_tier;
                    // Only save if score is better than previous best for this tier
                    if score > self.best_saved_scores[tier] {
                        let prev_score = self.best_saved_scores[tier];
                        let brain_clone = cell.brain.clone();
                        let generation = cell.generation;
                        let children = cell.children_count;
                        let energy = cell.energy_from_cells;
                        let age = cell.age;
                        crate::storage::save_best_neural_network(
                            tier,
                            &brain_clone,
                            generation,
                            score,
                            children,
                            energy,
                            age,
                        );
                        // Update cache and best score
                        self.cached_best_brains[tier] = Some((brain_clone, generation));
                        self.best_saved_scores[tier] = score;
                        println!(
                            "📈 New high score (tier {}): {:.1} (previous: {:.1})",
                            tier, score, prev_score
                        );
                    }
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

        // Calculate local density for each cell (must be done before parallel update)
        let density_counts: Vec<usize> = self
            .cells
            .iter()
            .map(|cell| self.spatial_grid.count_nearby_in_bucket(cell.x, cell.y))
            .collect();

        // Capture max_cells for density penalty calculation
        let max_cells = self.max_cells;

        // Update sensors for each cell in parallel
        self.cells.par_iter_mut().enumerate().for_each(|(i, cell)| {
            // Update local density from pre-calculated counts
            cell.local_density = density_counts[i];

            // Calculate density penalty if cluster > 50% of population cap
            if cell.local_density > max_cells / 2 {
                // Penalty: (1 - (1 / nb_cells))
                // This value is subtracted from score, so higher density = higher penalty
                cell.density_penalty = 1.0 - (1.0 / cell.local_density as f32);
            } else {
                // No penalty when not overcrowded
                cell.density_penalty = 0.0;
            }
            // Query nearby cells using spatial grid
            let nearby_indices = self.spatial_grid.query_nearby(cell.x, cell.y, SENSOR_RANGE);

            // Calculate distances and angles to all nearby cells
            let mut sensor_data: Vec<(usize, f32, f32, f32, f32, f32)> = nearby_indices
                .iter()
                .filter_map(|&j| {
                    if i == j {
                        return None; // Skip self
                    }

                    // Bounds check for safety
                    if j >= cell_data.len() {
                        return None;
                    }

                    let (x2, y2, energy, mass, is_alive) = cell_data[j];

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

                    let distance_squared = dx * dx + dy * dy;

                    // Filter out cells that are too far using squared distance to avoid sqrt()
                    if distance_squared > SENSOR_RANGE_SQUARED {
                        return None;
                    }

                    let distance = distance_squared.sqrt();

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

                    // Return (index, angle_from_front, distance, mass, is_alive, energy)
                    Some((j, angle_from_front, distance, mass, is_alive, energy))
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
                    let energy_a = a.5; // Energy is now at position 5
                    let energy_b = b.5;

                    // Sort by alive status ascending (0.0 before 1.0, so dead before alive)
                    // Then by energy descending, then by distance ascending
                    is_alive_a
                        .partial_cmp(&is_alive_b)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then(
                            energy_b
                                .partial_cmp(&energy_a)
                                .unwrap_or(std::cmp::Ordering::Equal),
                        )
                        .then(a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
                });
                // Keep only the top SENSOR_COUNT
                sensor_data.truncate(SENSOR_COUNT);
            }

            // Calculate center of mass for dead and alive cells separately
            let (dead_cells, alive_cells): (Vec<_>, Vec<_>) = sensor_data
                .iter()
                .partition(|&&(_, _, _, _, is_alive, _)| is_alive == 0.0);

            let dead_count = dead_cells.len() as f32;
            let alive_count = alive_cells.len() as f32;
            let total_count = dead_count + alive_count;

            // Calculate dead/alive ratio: -1.0 = all alive, 1.0 = all dead, 0.0 = balanced
            cell.dead_alive_ratio = if total_count > 0.0 {
                (dead_count - alive_count) / total_count
            } else {
                0.0
            };

            // Calculate center of mass for dead cells
            if !dead_cells.is_empty() {
                let mut sum_x = 0.0f32;
                let mut sum_y = 0.0f32;

                for &&(_j, angle_from_front, distance, _, _, _) in &dead_cells {
                    // Convert to cell's local coordinate frame (angle_from_front is already relative to facing direction)
                    sum_x += angle_from_front.cos() * distance;
                    sum_y += angle_from_front.sin() * distance;
                }

                let avg_x = sum_x / dead_count;
                let avg_y = sum_y / dead_count;

                // Calculate angle and distance to center of mass (in cell's local frame)
                cell.dead_center_distance = (avg_x * avg_x + avg_y * avg_y).sqrt();
                cell.dead_center_angle = avg_y.atan2(avg_x); // Relative to cell's facing direction
            } else {
                // No dead cells detected
                cell.dead_center_angle = 0.0;
                cell.dead_center_distance = SENSOR_RANGE; // Max range indicates nothing detected
            }

            // Calculate center of mass for alive cells
            if !alive_cells.is_empty() {
                let mut sum_x = 0.0f32;
                let mut sum_y = 0.0f32;

                for &&(_j, angle_from_front, distance, _, _, _) in &alive_cells {
                    // Convert to cell's local coordinate frame (angle_from_front is already relative to facing direction)
                    sum_x += angle_from_front.cos() * distance;
                    sum_y += angle_from_front.sin() * distance;
                }

                let avg_x = sum_x / alive_count;
                let avg_y = sum_y / alive_count;

                // Calculate angle and distance to center of mass (in cell's local frame)
                cell.alive_center_distance = (avg_x * avg_x + avg_y * avg_y).sqrt();
                cell.alive_center_angle = avg_y.atan2(avg_x); // Relative to cell's facing direction
            } else {
                // No alive cells detected
                cell.alive_center_angle = 0.0;
                cell.alive_center_distance = SENSOR_RANGE; // Max range indicates nothing detected
            }

            cell.nearest_cells = sensor_data;
        });
    }

    fn update_stats(&mut self) {
        // First, find the cell with the highest score (only alive cells)
        let mut best_score = f32::MIN;
        let mut best_cell_index = None;
        let mut alive_cells = Vec::new();

        for (i, cell) in self.cells.iter().enumerate() {
            // Only consider alive cells
            if cell.state == CellState::Alive {
                alive_cells.push(cell);

                // Use new comprehensive score method
                let score = cell.score();
                if score > best_score {
                    best_score = score;
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

        // Calculate per-tier cell counts and intra-tier hue diversity
        let mut tier_counts = [0usize; 4];
        let mut tier_hues: [Vec<f32>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        for cell in &alive_cells {
            let tier = cell.brain_tier.min(3);
            tier_counts[tier] += 1;
            let (h, _, _) = Cell::rgb_to_hsv_public(cell.color);
            tier_hues[tier].push(h);
        }
        self.tier_cell_counts = tier_counts;
        for (tier, hues) in tier_hues.iter().enumerate() {
            self.tier_diversities[tier] = if hues.len() > 1 {
                let mean: f32 = hues.iter().sum::<f32>() / hues.len() as f32;
                let variance: f32 = hues
                    .iter()
                    .map(|h| {
                        let diff = (h - mean).abs();
                        let diff = if diff > 180.0 { 360.0 - diff } else { diff };
                        diff * diff
                    })
                    .sum::<f32>()
                    / hues.len() as f32;
                (variance / (180.0 * 180.0)).min(1.0)
            } else {
                0.0
            };
        }

        // Update stats and genome with the best cell only, or clear if no alive cells
        let current_time = get_time();

        // Check if the currently followed cell has died
        let should_switch_target = if let Some(last_index) = self.last_best_cell_index {
            if last_index < self.cells.len() && self.cells[last_index].state == CellState::Alive {
                // Currently followed cell is still alive, reset death timer
                self.followed_cell_death_time = None;
                true // Can switch immediately to a better cell
            } else {
                // Currently followed cell is dead
                if self.followed_cell_death_time.is_none() {
                    // Record death time
                    self.followed_cell_death_time = Some(current_time);
                }

                // Check if 3 seconds have passed since death
                if let Some(death_time) = self.followed_cell_death_time {
                    current_time - death_time >= 3.0
                } else {
                    false
                }
            }
        } else {
            // No cell being followed, can switch immediately
            true
        };

        if let Some(best_index) = best_cell_index {
            let best_cell = &self.cells[best_index];

            // Only update to new best cell if we should switch targets
            if should_switch_target {
                // Only clone the best cell's genome if it changed (avoid expensive clone every frame)
                if self.last_best_cell_index != Some(best_index) {
                    self.best_cell_genome = Some(best_cell.clone());
                    self.last_best_cell_index = Some(best_index);
                    self.followed_cell_death_time = None; // Reset death timer for new target
                }

                // Set stats to show the current best alive cell
                self.stats.set(crate::stats::BestCellStats {
                    energy_from_cells: best_cell.energy_from_cells,
                    current_energy: best_cell.energy,
                    children_count: best_cell.children_count,
                    generation: best_cell.generation,
                    color: best_cell.color,
                    age: best_cell.age,
                    x: best_cell.x,
                    y: best_cell.y,
                    is_alive: true, // is_alive is guaranteed true since we filtered for it
                    brain_tier: best_cell.brain_tier,
                    brain_operations: best_cell.brain.operation_count(),
                });

                // Update selected cell index if stats are selected
                if self.stats.is_selected() {
                    self.selected_cell_index = Some(best_index);
                } else {
                    self.selected_cell_index = None;
                }
            } else if let Some(last_index) = self.last_best_cell_index {
                // Keep showing the dead cell until 3 seconds pass
                if last_index < self.cells.len() {
                    let dead_cell = &self.cells[last_index];
                    self.stats.set(crate::stats::BestCellStats {
                        energy_from_cells: dead_cell.energy_from_cells,
                        current_energy: dead_cell.energy,
                        children_count: dead_cell.children_count,
                        generation: dead_cell.generation,
                        color: dead_cell.color,
                        age: dead_cell.age,
                        x: dead_cell.x,
                        y: dead_cell.y,
                        is_alive: false,
                        brain_tier: dead_cell.brain_tier,
                        brain_operations: dead_cell.brain.operation_count(),
                    });
                }
            }
        } else if should_switch_target {
            // No alive cells found and cooldown has passed, clear the stats
            self.stats.clear();
            self.last_best_cell_index = None;
            self.selected_cell_index = None;
            self.followed_cell_death_time = None;
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
        let world_width = self.config.world_width;
        let world_height = self.config.world_height;
        let screen_w = screen_width();
        let screen_h = screen_height();

        // Define all possible wraparound positions (same as cell rendering)
        let wraparound_offsets = [
            (0.0, 0.0),                    // Original position
            (-world_width, 0.0),           // Wrapped left
            (world_width, 0.0),            // Wrapped right
            (0.0, -world_height),          // Wrapped top
            (0.0, world_height),           // Wrapped bottom
            (-world_width, -world_height), // Top-left corner
            (world_width, -world_height),  // Top-right corner
            (-world_width, world_height),  // Bottom-left corner
            (world_width, world_height),   // Bottom-right corner
        ];

        for cell in &self.cells {
            // Only draw sensors for alive cells
            if cell.state != CellState::Alive {
                continue;
            }

            // Render sensor lines for each wraparound position
            for (offset_x, offset_y) in &wraparound_offsets {
                // Adjust camera position to create wraparound effect
                let adjusted_camera_x = self.camera.x - offset_x;
                let adjusted_camera_y = self.camera.y - offset_y;

                // Check if cell is visible at this wraparound position
                let cell_screen_x = cell.x - adjusted_camera_x;
                let cell_screen_y = cell.y - adjusted_camera_y;
                let margin = SENSOR_RANGE; // Use sensor range as margin

                if cell_screen_x < -margin
                    || cell_screen_x > screen_w + margin
                    || cell_screen_y < -margin
                    || cell_screen_y > screen_h + margin
                {
                    continue; // Skip if cell not visible at this wraparound position
                }

                for &(target_idx, _angle, _distance, _mass, _is_alive, _energy) in
                    &cell.nearest_cells
                {
                    // Safety check: ensure target index is valid
                    if target_idx >= self.cells.len() {
                        continue;
                    }

                    let target = &self.cells[target_idx];

                    // Calculate vector from cell to target
                    let mut dx = target.x - cell.x;
                    let mut dy = target.y - cell.y;

                    // Handle world wrapping for line drawing
                    if dx.abs() > world_width / 2.0 {
                        dx = dx - dx.signum() * world_width;
                    }
                    if dy.abs() > world_height / 2.0 {
                        dy = dy - dy.signum() * world_height;
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

                    // Convert to screen coordinates using adjusted camera
                    let x1 = cell.x - adjusted_camera_x;
                    let y1 = cell.y - adjusted_camera_y;
                    let x2 = target_x - adjusted_camera_x;
                    let y2 = target_y - adjusted_camera_y;

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
    }

    pub fn render(&self) {
        // Render parallax star-field background
        if let Some(bg) = &self.background {
            bg.render(self.camera.x, self.camera.y);
        }

        // Render boundary lines (only if UI enabled)
        if self.config.show_ui {
            self.render_grid();
            self.render_boundaries();
        }

        // Render sensor lines first (so they appear behind cells)
        if self.config.show_sensor_lines {
            self.render_sensor_lines();
        }

        // Count stats
        let mut cells_in_viewport = 0;
        let screen_w = screen_width();
        let screen_h = screen_height();

        // Get world dimensions for wraparound rendering
        let world_width = self.config.world_width;
        let world_height = self.config.world_height;

        // Define all possible wraparound positions
        // Cells are rendered at multiple positions to show toroidal topology
        let wraparound_offsets = [
            (0.0, 0.0),                    // Original position
            (-world_width, 0.0),           // Wrapped left
            (world_width, 0.0),            // Wrapped right
            (0.0, -world_height),          // Wrapped top
            (0.0, world_height),           // Wrapped bottom
            (-world_width, -world_height), // Top-left corner
            (world_width, -world_height),  // Top-right corner
            (-world_width, world_height),  // Bottom-left corner
            (world_width, world_height),   // Bottom-right corner
        ];

        // Render cells and count viewport cells
        for (idx, cell) in self.cells.iter().enumerate() {
            // Count cells in viewport (only for original position)
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

            // Render cell at all wraparound positions
            for (dx, dy) in &wraparound_offsets {
                // Adjust camera position to create wraparound effect
                let adjusted_camera_x = self.camera.x - dx;
                let adjusted_camera_y = self.camera.y - dy;

                // cell.render() has built-in viewport culling, will skip if off-screen
                cell.render(adjusted_camera_x, adjusted_camera_y);

                // Draw selection highlight if this is the selected cell
                if self.selected_cell_index == Some(idx) {
                    let screen_x = cell.x - adjusted_camera_x;
                    let screen_y = cell.y - adjusted_camera_y;
                    let current_radius = cell.get_current_radius();

                    // Only draw if on screen
                    let margin = current_radius * 1.5;
                    if !(screen_x < -margin
                        || screen_x > screen_w + margin
                        || screen_y < -margin
                        || screen_y > screen_h + margin)
                    {
                        let gold = Color::new(1.0, 0.84, 0.0, 1.0);
                        let gold_transparent = Color::new(1.0, 0.84, 0.0, 0.5);
                        draw_circle_lines(
                            screen_x,
                            screen_y,
                            current_radius + 9.0, // Slightly larger than cell
                            4.0,                  // Thickness
                            gold_transparent,
                        );
                        draw_circle_lines(
                            screen_x,
                            screen_y,
                            current_radius + 12.0, // Slightly larger than cell
                            1.0,                   // Thickness
                            gold,
                        );

                        // Draw target line if cell has a target
                        if let Some((target_x, target_y)) = cell.current_target_pos {
                            // Calculate nose position (front of the cell)
                            let nose_x = screen_x + cell.angle.cos() * current_radius;
                            let nose_y = screen_y + cell.angle.sin() * current_radius;

                            // Calculate target position accounting for world wrapping
                            let mut target_dx = target_x - cell.x;
                            let mut target_dy = target_y - cell.y;

                            // Adjust for world wrapping
                            if target_dx.abs() > world_width / 2.0 {
                                target_dx = target_dx - target_dx.signum() * world_width;
                            }
                            if target_dy.abs() > world_height / 2.0 {
                                target_dy = target_dy - target_dy.signum() * world_height;
                            }

                            let target_screen_x = screen_x + target_dx;
                            let target_screen_y = screen_y + target_dy;

                            // Calculate line color based on current_alignment_score
                            // Score ranges from 1.0 (0° diff, perfect) to -1.0 (180° diff, opposite)
                            // Color mapping:
                            //   1.0 (0° diff) = White (perfectly aligned)
                            //   0.0 (90° diff) = Yellow (perpendicular)
                            //   -1.0 (180° diff) = Red (facing away)
                            let alignment = cell.current_alignment_score.clamp(-1.0, 1.0);

                            let line_color = if alignment >= 0.0 {
                                // 0° to 90°: interpolate from white (1.0) to yellow (0.0)
                                // alignment = 1.0: white (1,1,1)
                                // alignment = 0.0: yellow (1,1,0)
                                Color::new(1.0, 1.0, alignment, 0.9)
                            } else {
                                // 90° to 180°: interpolate from yellow (0.0) to red (-1.0)
                                // alignment = 0.0: yellow (1,1,0)
                                // alignment = -1.0: red (1,0,0)
                                let t = -alignment; // 0.0 to 1.0
                                Color::new(1.0, 1.0 - t, 0.0, 0.9)
                            };

                            // Draw line from nose to target
                            draw_line(
                                nose_x,
                                nose_y,
                                target_screen_x,
                                target_screen_y,
                                3.0,
                                line_color,
                            );

                            // Draw small circle at target position
                            draw_circle(target_screen_x, target_screen_y, 5.0, line_color);
                        }

                        // Draw center of mass indicators for dead and alive cells
                        // Light gray for dead cells, yellow for alive cells
                        const CENTER_OF_MASS_RADIUS: f32 = 8.0;

                        // Dead cells center of mass (light gray)
                        if cell.dead_center_distance < SENSOR_RANGE {
                            // Calculate screen position from cell's local frame
                            let dead_center_world_angle = cell.angle + cell.dead_center_angle;
                            let dead_center_world_x =
                                cell.x + dead_center_world_angle.cos() * cell.dead_center_distance;
                            let dead_center_world_y =
                                cell.y + dead_center_world_angle.sin() * cell.dead_center_distance;

                            let dead_center_screen_x = dead_center_world_x - adjusted_camera_x;
                            let dead_center_screen_y = dead_center_world_y - adjusted_camera_y;

                            let light_gray = Color::new(0.7, 0.7, 0.7, 0.8);
                            draw_circle(
                                dead_center_screen_x,
                                dead_center_screen_y,
                                CENTER_OF_MASS_RADIUS,
                                light_gray,
                            );
                            draw_circle_lines(
                                dead_center_screen_x,
                                dead_center_screen_y,
                                CENTER_OF_MASS_RADIUS,
                                2.0,
                                Color::new(0.5, 0.5, 0.5, 1.0),
                            );
                        }

                        // Alive cells center of mass (yellow)
                        if cell.alive_center_distance < SENSOR_RANGE {
                            // Calculate screen position from cell's local frame
                            let alive_center_world_angle = cell.angle + cell.alive_center_angle;
                            let alive_center_world_x = cell.x
                                + alive_center_world_angle.cos() * cell.alive_center_distance;
                            let alive_center_world_y = cell.y
                                + alive_center_world_angle.sin() * cell.alive_center_distance;

                            let alive_center_screen_x = alive_center_world_x - adjusted_camera_x;
                            let alive_center_screen_y = alive_center_world_y - adjusted_camera_y;

                            let yellow = Color::new(1.0, 1.0, 0.0, 0.8);
                            draw_circle(
                                alive_center_screen_x,
                                alive_center_screen_y,
                                CENTER_OF_MASS_RADIUS,
                                yellow,
                            );
                            draw_circle_lines(
                                alive_center_screen_x,
                                alive_center_screen_y,
                                CENTER_OF_MASS_RADIUS,
                                2.0,
                                Color::new(0.8, 0.8, 0.0, 1.0),
                            );
                        }
                    }
                }
            }
        }

        // Render stats (only if UI enabled)
        if self.config.show_ui {
            self.render_stats(cells_in_viewport);

            // Render best cell stats (top-right corner)
            self.stats.render(self.font.as_ref());
        }
    }

    fn render_grid(&self) {
        let grid_spacing = 250.0;
        let dot_radius = 2.0;
        let dot_color = Color::new(1.0, 1.0, 1.0, 0.3); // White with low opacity

        let screen_w = screen_width();
        let screen_h = screen_height();

        // Get world dimensions for wraparound rendering
        let world_width = self.config.world_width;
        let world_height = self.config.world_height;

        // Define all possible wraparound positions (same as cell rendering)
        let wraparound_offsets = [
            (0.0, 0.0),                    // Original position
            (-world_width, 0.0),           // Wrapped left
            (world_width, 0.0),            // Wrapped right
            (0.0, -world_height),          // Wrapped top
            (0.0, world_height),           // Wrapped bottom
            (-world_width, -world_height), // Top-left corner
            (world_width, -world_height),  // Top-right corner
            (-world_width, world_height),  // Bottom-left corner
            (world_width, world_height),   // Bottom-right corner
        ];

        // Calculate which grid points are visible on screen
        // Start from the first grid point that could be visible
        let start_x = ((self.camera.x / grid_spacing).floor() * grid_spacing).max(0.0);
        let start_y = ((self.camera.y / grid_spacing).floor() * grid_spacing).max(0.0);

        // Draw grid points within world boundaries at all wraparound positions
        let mut y = start_y;
        while y <= world_height {
            let mut x = start_x;
            while x <= world_width {
                // Render dot at all wraparound positions
                for (dx, dy) in &wraparound_offsets {
                    // Adjust camera position to create wraparound effect
                    let adjusted_camera_x = self.camera.x - dx;
                    let adjusted_camera_y = self.camera.y - dy;

                    // Convert world coordinates to screen coordinates
                    let screen_x = x - adjusted_camera_x;
                    let screen_y = y - adjusted_camera_y;

                    // Only draw if on screen
                    if screen_x >= -dot_radius
                        && screen_x <= screen_w + dot_radius
                        && screen_y >= -dot_radius
                        && screen_y <= screen_h + dot_radius
                    {
                        draw_circle(screen_x, screen_y, dot_radius, dot_color);
                    }
                }

                x += grid_spacing;
            }
            y += grid_spacing;
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

        // Lines 5-9: Per-tier population bars + total
        let bar_max_width = 200.0_f32;
        let bar_height = 14.0_f32;
        // Base hue per tier: 180 + tier * 90 (same as Cell::spawn)
        let tier_hues = [180.0_f32, 270.0, 0.0, 90.0];
        let total_alive = self.tier_cell_counts.iter().sum::<usize>().max(1);

        for (tier, &tier_hue) in tier_hues.iter().enumerate() {
            let y_base = padding + font_size + line_height * (4.0 + tier as f32);
            let count = self.tier_cell_counts[tier];
            let diversity_pct = self.tier_diversities[tier] * 100.0;

            // Tier label color derived from base hue (s=0.8, v=0.9)
            let h = tier_hue;
            let tier_color = {
                let s = 0.8_f32;
                let v = 0.9_f32;
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
            };

            let label = format!("m{}:", tier);
            let label_width = measure_text(&label, self.font.as_ref(), font_size as u16, 1.0).width;
            draw_text_ex(
                &label,
                padding,
                y_base,
                TextParams {
                    font: self.font.as_ref(),
                    font_size: font_size as u16,
                    color: tier_color,
                    ..Default::default()
                },
            );

            // Progress bar
            let bar_x = padding + label_width + 6.0;
            let bar_y = y_base - bar_height + 2.0;
            let fill_width = (count as f32 / total_alive as f32) * bar_max_width;
            // Background track
            draw_rectangle(
                bar_x,
                bar_y,
                bar_max_width,
                bar_height,
                Color::new(1.0, 1.0, 1.0, 0.1),
            );
            // Filled portion
            if fill_width > 0.0 {
                draw_rectangle(
                    bar_x,
                    bar_y,
                    fill_width,
                    bar_height,
                    Color::new(tier_color.r, tier_color.g, tier_color.b, 0.7),
                );
            }

            // Count + diversity text
            let info = format!(" {} cells ({:.0}%)", count, diversity_pct);
            draw_text_ex(
                &info,
                bar_x + bar_max_width + 4.0,
                y_base,
                TextParams {
                    font: self.font.as_ref(),
                    font_size: font_size as u16,
                    color: WHITE,
                    ..Default::default()
                },
            );
        }

        // Total line
        let total_diversity = self.tier_diversities.iter().sum::<f32>() / 4.0;
        let total_y = padding + font_size + line_height * 8.0;
        let total_text = format!(
            "total: {} cells ({:.0}% diversity)",
            total_alive,
            total_diversity * 100.0
        );
        draw_text_ex(
            &total_text,
            padding,
            total_y,
            TextParams {
                font: self.font.as_ref(),
                font_size: font_size as u16,
                color: Color::new(0.8, 0.8, 0.8, 1.0),
                ..Default::default()
            },
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
