use crate::neural_network::NeuralNetwork;
use macroquad::prelude::*;

// Cell behavior constants
const CONSTANT_FORWARD_FORCE: f32 = 0.1;
const METABOLISM_ENERGY_LOSS: f32 = 0.03;
const CORPSE_DECAY_RATE: f32 = 0.02;
// Hunger: metabolism multiplier grows the longer a cell goes without eating.
// Ramps from 1x up to HUNGER_MAX_MULTIPLIER over HUNGER_RAMP_TICKS ticks.
const HUNGER_RAMP_TICKS: f32 = 300.0;
const HUNGER_MAX_MULTIPLIER: f32 = 4.0;
const GROWTH_AGE_THRESHOLD: f32 = 20.0;
const ADULT_AGE_THRESHOLD: f32 = 30.0;
const MIN_RADIUS_PERCENT: f32 = 0.1;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CellState {
    Alive,
    Corpse,
}

#[derive(Clone)]
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

    // ===== Stats Tracking =====
    pub total_energy_accumulated: f32, // Total energy gained throughout lifetime
    pub energy_from_cells: f32,        // Energy gained specifically from reaching other cells
    pub children_count: usize,         // Number of children produced
    pub generation: usize,             // Generation count (0 for initial, 1+ for descendants)
    pub ticks_since_last_fed: f32,     // Drives hunger multiplier on metabolism
    pub tracking_score: f32,           // Accumulated reward for turning toward corpses
    pub prev_target_angle: Option<f32>, // Previous angle to target (for tracking improvement)
    pub current_target_pos: Option<(f32, f32)>, // Current target position for debugging visualization
    pub current_alignment_score: f32, // Current alignment score: 1.0 at 0°, 0.0 at 90°, -1.0 at 180°
    pub last_action: Option<u8>, // Last action taken: 0=noop, 1=turn_left, 2=turn_right, 3=forward

    // ===== Sensors =====
    // Each sensor returns: (cell_index, angle_from_front, distance, mass, is_alive, energy)
    // angle_from_front: -180..180 degrees relative to cell's facing direction
    // distance: 0..200 units
    // mass: target cell's mass (energy capacity)
    // is_alive: 1.0 if alive, 0.0 if dead/corpse
    // energy: target cell's current energy
    pub nearest_cells: Vec<(usize, f32, f32, f32, f32, f32)>, // (index, angle, distance, mass, is_alive, energy) for 5 nearest cells

    // Center of mass sensors (calculated from nearest_cells)
    pub dead_alive_ratio: f32, // -1.0 = all alive, 1.0 = all dead, 0.0 = balanced
    pub dead_center_angle: f32, // Angle to dead cells center of mass (radians, -PI to PI)
    pub dead_center_distance: f32, // Distance to dead cells center of mass
    pub alive_center_angle: f32, // Angle to alive cells center of mass (radians, -PI to PI)
    pub alive_center_distance: f32, // Distance to alive cells center of mass

    // Density sensor (calculated from spatial grid)
    pub local_density: usize, // Number of cells in same bucket + neighboring buckets (includes self)
    pub density_penalty: f32, // Penalty applied when cluster > 50% of population cap

    // ===== Neural Network Brain =====
    pub brain: NeuralNetwork,
    pub brain_tier: usize, // 0-3: determines hidden layer width and hue offset

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
        Self::rgb_to_hsv_public(color)
    }

    // Public version for diversity calculation
    pub fn rgb_to_hsv_public(color: Color) -> (f32, f32, f32) {
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
    // Age 0-ADULT_AGE_THRESHOLD: scales from MIN_RADIUS_PERCENT to 100% of base radius
    // Age ADULT_AGE_THRESHOLD+: stays at 100%
    pub fn get_current_radius(&self) -> f32 {
        if self.age < ADULT_AGE_THRESHOLD {
            let size_percent =
                MIN_RADIUS_PERCENT + (self.age / ADULT_AGE_THRESHOLD) * (1.0 - MIN_RADIUS_PERCENT);
            self.radius * size_percent
        } else {
            self.radius
        }
    }

    pub fn spawn(
        world_width: f32,
        world_height: f32,
        brain_tier: usize,
        cached_brain: &Option<(NeuralNetwork, usize)>,
    ) -> Self {
        let speed = rand::gen_range(0.2, 1.0);
        let angle = rand::gen_range(0.0, std::f32::consts::TAU);
        // Energy chunk size: 50 ± 10% = 45 to 55
        let energy_chunk_size = rand::gen_range(45.0, 55.0);
        // Species multiplier: how efficiently energy is extracted (0.9x to 2.0x)
        let species_multiplier = rand::gen_range(0.9, 2.0);
        // Mass: max energy capacity, around 200 ± 10%
        let mass = rand::gen_range(180.0, 220.0);

        let hidden_multiplier = brain_tier + 1;

        // Use cached brain if available, otherwise create a new random network
        let (brain, loaded_generation) = if let Some((saved_brain, generation)) = cached_brain {
            let mut brain = saved_brain.clone();
            // Apply small mutation (1-5%) to add variance
            let mutation_rate = rand::gen_range(0.01, 0.05);
            brain.mutate(mutation_rate);
            (brain, *generation)
        } else {
            // No cached brain, create new random network with tier-appropriate size
            (
                NeuralNetwork::new_with_multiplier(27, 4, hidden_multiplier),
                0,
            )
        };

        // Hue offset: 0° for tier 0, +90° for each subsequent tier
        let base_hue = (180.0 + brain_tier as f32 * 90.0).rem_euclid(360.0);

        Cell {
            // Individual State
            x: rand::gen_range(0.0, world_width),
            y: rand::gen_range(0.0, world_height),
            velocity_x: angle.cos() * speed * rand::gen_range(0.5, 1.0),
            velocity_y: angle.sin() * speed * rand::gen_range(0.5, 1.0),
            energy: 100.0,
            angle,
            angle_velocity: rand::gen_range(-0.05, 0.05),
            state: CellState::Alive,
            age: 0.0,

            // Stats Tracking
            total_energy_accumulated: 100.0, // Start with initial energy
            energy_from_cells: 0.0,          // No energy from cells yet
            children_count: 0,
            generation: loaded_generation, // Use loaded generation from saved brain
            ticks_since_last_fed: 0.0,
            tracking_score: 0.0,
            prev_target_angle: None,
            current_target_pos: None,
            current_alignment_score: 0.0,
            last_action: None,

            // Sensors
            nearest_cells: Vec::new(),
            dead_alive_ratio: 0.0,
            dead_center_angle: 0.0,
            dead_center_distance: 200.0, // Default to max range (nothing detected)
            alive_center_angle: 0.0,
            alive_center_distance: 200.0,
            local_density: 1,     // Will be updated on first sensor update
            density_penalty: 0.0, // Will be updated on first sensor update

            // Neural Network Brain
            brain,
            brain_tier,

            // Inherited Attributes
            color: Self::hsv_to_rgb(base_hue, 0.8, 0.9),
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
        let mutated_speed = Self::mutate(self.speed, 0.2, 1.0);

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

            // Stats Tracking
            total_energy_accumulated: 0.0, // Start fresh
            energy_from_cells: 0.0,        // No energy from cells yet
            children_count: 0,
            generation: self.generation + 1, // Increment generation
            ticks_since_last_fed: 0.0,
            tracking_score: 0.0,
            prev_target_angle: None,
            current_target_pos: None,
            current_alignment_score: 0.0,
            last_action: None,

            // Sensors
            nearest_cells: Vec::new(),
            dead_alive_ratio: 0.0,
            dead_center_angle: 0.0,
            dead_center_distance: 200.0, // Default to max range (nothing detected)
            alive_center_angle: 0.0,
            alive_center_distance: 200.0,
            local_density: 1,     // Will be updated on first sensor update
            density_penalty: 0.0, // Will be updated on first sensor update

            // Neural Network Brain (inherited and mutated)
            brain,
            brain_tier: self.brain_tier,

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
    // Plus 1 value for current energy level
    // Plus 5 values for center of mass (dead/alive ratio, dead angle/distance, alive angle/distance)
    // Plus 1 value for local density (1 / nb_cells in bucket cluster)
    // Total: 5 sensors × 4 values + 1 energy + 5 center of mass + 1 density = 27 inputs
    fn normalize_sensors(&self) -> Vec<f32> {
        use crate::world::{DEPLETED_CELL_ENERGY, REPRODUCTION_ENERGY_THRESHOLD, SENSOR_RANGE};
        const MAX_MASS: f32 = 220.0; // Maximum mass value from spawn()
        let mut inputs = Vec::with_capacity(27);

        for i in 0..5 {
            if i < self.nearest_cells.len() {
                let (_index, angle, distance, mass, is_alive, _energy) = self.nearest_cells[i];

                // Angle: -PI..PI -> -1..1
                let normalized_angle = angle / std::f32::consts::PI;

                // Distance: 0..SENSOR_RANGE -> -1..1 (closer = higher value)
                let normalized_distance = (SENSOR_RANGE - distance) / SENSOR_RANGE;
                let normalized_distance = normalized_distance * 2.0 - 1.0;

                // Mass: 0..MAX_MASS -> -1..1 (normalized around expected range)
                let normalized_mass = (mass / MAX_MASS) * 2.0 - 1.0;

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

        // Add current energy level as input
        // Normalize: DEPLETED_CELL_ENERGY (-100) -> 0.0, REPRODUCTION_ENERGY_THRESHOLD (100) -> 1.0
        let energy_range = REPRODUCTION_ENERGY_THRESHOLD - DEPLETED_CELL_ENERGY;
        let normalized_energy =
            ((self.energy - DEPLETED_CELL_ENERGY) / energy_range).clamp(0.0, 1.0);
        inputs.push(normalized_energy);

        // Add center of mass inputs (5 values)
        // Dead/alive ratio: -1.0 = all alive cells, 1.0 = all dead cells, 0.0 = balanced
        inputs.push(self.dead_alive_ratio);

        // Dead cells center of mass
        // Angle: -PI..PI -> -1..1
        let normalized_dead_angle = self.dead_center_angle / std::f32::consts::PI;
        inputs.push(normalized_dead_angle);
        // Distance: 0..SENSOR_RANGE -> 1..-1 (closer = higher value)
        let normalized_dead_distance = (SENSOR_RANGE - self.dead_center_distance) / SENSOR_RANGE;
        let normalized_dead_distance = normalized_dead_distance * 2.0 - 1.0;
        inputs.push(normalized_dead_distance);

        // Alive cells center of mass
        // Angle: -PI..PI -> -1..1
        let normalized_alive_angle = self.alive_center_angle / std::f32::consts::PI;
        inputs.push(normalized_alive_angle);
        // Distance: 0..SENSOR_RANGE -> 1..-1 (closer = higher value)
        let normalized_alive_distance = (SENSOR_RANGE - self.alive_center_distance) / SENSOR_RANGE;
        let normalized_alive_distance = normalized_alive_distance * 2.0 - 1.0;
        inputs.push(normalized_alive_distance);

        // Local density: 1 / nb_cells (higher value = less crowded)
        // Ensures value is always > 0 and <= 1.0
        let density_input = 1.0 / self.local_density.max(1) as f32;
        inputs.push(density_input);

        inputs
    }

    // Make a decision using the neural network
    // Actions: 0 = no-op, 1 = turn_left, 2 = turn_right, 3 = forward
    fn decide_action(&mut self) {
        let inputs = self.normalize_sensors();
        let action = self.brain.get_best_action(&inputs);

        // Store the action taken for reward calculation
        self.last_action = Some(action as u8);

        match action {
            0 => {} // No-op (do nothing)
            1 => self.turn_left(),
            2 => self.turn_right(),
            3 => self.forward(),
            _ => {} // Should never happen, but handle gracefully
        }
    }

    pub fn update(&mut self, world_width: f32, world_height: f32) {
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
            let diff = self.energy - self.mass;
            self.tracking_score += diff;
            self.energy = self.mass;
        }

        // Passive energy loss for all cells
        if self.state == CellState::Alive {
            // Hunger: metabolism scales up the longer a cell goes without eating,
            // pressuring cells to actively seek food rather than drift passively.
            self.ticks_since_last_fed += 1.0;
            let hunger_multiplier = (1.0
                + (self.ticks_since_last_fed / HUNGER_RAMP_TICKS) * (HUNGER_MAX_MULTIPLIER - 1.0))
                .min(HUNGER_MAX_MULTIPLIER);
            self.energy -= METABOLISM_ENERGY_LOSS * hunger_multiplier;

            // Use neural network to decide action instead of random movement
            self.decide_action();

            // Reward alignment toward targets each tick.
            // Priority: dead cells (corpses) first, then weaker live cells if no corpses.
            // angle_from_front is already atan2(target)-cell_angle wrapped to -PI..PI.
            // Logarithmic reward within ±90°: precision near 0° is worth more.
            // Extreme quadratic penalty beyond ±90°.

            // Try to find a corpse first (highest mass corpse)
            let corpse_data = self
                .nearest_cells
                .iter()
                .filter(|&&(_, _, _, _, is_alive, _)| is_alive == 0.0)
                .max_by(|&&(_, _, _, mass_a, _, _), &&(_, _, _, mass_b, _, _)| {
                    mass_a
                        .partial_cmp(&mass_b)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|&(_, angle, distance, _, _, _)| (angle, distance));

            // If no corpse, find a weaker live cell (lowest energy alive cell with energy < self.energy)
            let weak_prey_data = if corpse_data.is_none() {
                self.nearest_cells
                    .iter()
                    .filter(|&&(_, _, _, _, is_alive, energy)| {
                        is_alive == 1.0 && energy < self.energy
                    })
                    .min_by(|&&(_, _, _, _, _, energy_a), &&(_, _, _, _, _, energy_b)| {
                        energy_a
                            .partial_cmp(&energy_b)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|&(_, angle, distance, _, _, _)| (angle, distance))
            } else {
                None
            };

            // Use corpse if available, otherwise use weak prey
            let target_data = corpse_data.or(weak_prey_data);

            if let Some((curr_angle, distance)) = target_data {
                let abs_angle = curr_angle.abs();

                // Calculate if alignment improved since last frame
                let alignment_improved = if let Some(prev_angle) = self.prev_target_angle {
                    prev_angle.abs() - abs_angle // Positive = better, negative = worse
                } else {
                    0.0 // First frame, no comparison
                };

                // Check if a turn action was taken this frame
                let turn_action_taken = matches!(self.last_action, Some(1) | Some(2));
                // Check if forward action was taken
                let forward_action_taken = matches!(self.last_action, Some(3));

                // Action-based reward logic
                if turn_action_taken {
                    if alignment_improved > 0.0 {
                        // Reward for successful turning that improved alignment
                        // Scale reward by how much improvement was made
                        self.tracking_score += alignment_improved * 100.0;
                    }
                    // No penalty for trying - encourages exploration
                } else if forward_action_taken {
                    // Reward forward movement when aligned with target (within ±90°)
                    let half_turn = std::f32::consts::FRAC_PI_2;
                    if abs_angle < half_turn {
                        // Scale reward by alignment quality: 0° = max reward, 90° = 0 reward
                        // Linear scaling: (1 - angle/90°) gives 1.0 at 0°, 0.0 at 90°
                        let alignment_factor = 1.0 - (abs_angle / half_turn);
                        self.tracking_score += alignment_factor * 50.0;
                    }
                    // No penalty for forward when not aligned - just no reward
                } else {
                    if alignment_improved < 0.0 {
                        // Penalty for passive drift away from target
                        self.tracking_score += alignment_improved * 50.0; // Negative value
                    }
                    // No reward for passive good alignment - prevents infinite accumulation
                }

                // Keep existing excessive turning penalty
                let excessive_turn_penalty =
                    -(self.angle_velocity.abs().max(0.1) - 0.1).powi(2) * 50.0;
                self.tracking_score += excessive_turn_penalty;

                // Calculate current alignment score for visualization
                // Based solely on current angle difference:
                // 0° difference: +1.0 (perfectly aligned, white)
                // 90° difference: 0.0 (perpendicular, yellow)
                // 180° difference: -1.0 (opposite direction, red)
                self.current_alignment_score = 1.0 - (abs_angle / std::f32::consts::FRAC_PI_2);

                // Calculate target position for visualization (only if within 500 units)
                if distance <= 500.0 {
                    let target_world_angle = self.angle + curr_angle;
                    let target_x = self.x + target_world_angle.cos() * distance;
                    let target_y = self.y + target_world_angle.sin() * distance;
                    self.current_target_pos = Some((target_x, target_y));
                } else {
                    self.current_target_pos = None;
                }

                // Store current angle for next frame comparison
                self.prev_target_angle = Some(curr_angle);
            } else {
                self.current_alignment_score = 0.0;
                self.current_target_pos = None;
                self.prev_target_angle = None;
            }
        } else if self.state == CellState::Corpse {
            // Corpse decay: lose energy per tick
            self.energy -= CORPSE_DECAY_RATE;
        }

        // Constant slow forward movement for alive cells
        if self.state == CellState::Alive {
            self.velocity_x += self.angle.cos() * CONSTANT_FORWARD_FORCE;
            self.velocity_y += self.angle.sin() * CONSTANT_FORWARD_FORCE;
        }

        // Apply mass-based velocity slowdown
        // Higher mass = slower movement (mass acts as inertia/drag)
        let mass_factor = 200.0 / self.mass; // Normalize around 200
        let slowdown = mass_factor.max(0.5); // Don't slow down too much

        // Always apply velocity (drifting continues even when dead)
        self.x += self.velocity_x * slowdown;
        self.y += self.velocity_y * slowdown;
        self.angle += self.angle_velocity;

        // Boundary wrapping (inline instead of separate pass)
        self.x = self.x.rem_euclid(world_width);
        self.y = self.y.rem_euclid(world_height);

        self.velocity_y *= 0.95; // Friction
        self.velocity_x *= 0.95; // Friction
        self.angle_velocity *= 0.9; // Rotational friction
    }

    // Called when cell gains energy (from feeding)
    // For young cells (age < GROWTH_AGE_THRESHOLD), energy is lost to growth
    pub fn gain_energy(&mut self, amount: f32) {
        // Reset hunger counter — cell has found food
        self.ticks_since_last_fed = 0.0;

        // Track total energy accumulated
        self.total_energy_accumulated += amount;

        // Track energy specifically from cells (collisions)
        self.energy_from_cells += amount;

        if self.age < GROWTH_AGE_THRESHOLD {
            // Young cells: energy goes to growth, not stored
            // Energy is simply discarded (used for growing)
            return;
        }

        // Mature cells: energy is stored
        self.energy += amount;
    }

    // Calculate cell's comprehensive fitness score
    // Priority: children count (primary), energy from cells (equally important), age (secondary)
    pub fn score(&self) -> f32 {
        // Children count: 100 points per child (primary metric)
        let children_score = self.children_count as f32 * 100.0;

        // Energy from cells: 1 point per energy (equally important as children)
        let energy_score = self.energy_from_cells;

        // Age: 10 points per age unit (secondary metric - older cells have survived longer)
        let age_score = self.age * 10.0;

        // Tracking: reward accumulated angle-improvement toward corpses.
        // Scale by 50 so ~100 ticks of good tracking ≈ half a child's worth of score.
        let tracking = self.tracking_score * 50.0;

        // Density penalty: discourage overcrowding (penalty applied in world.rs when cluster > 50% of cap)
        let density_penalty_score = self.density_penalty;

        children_score + energy_score + age_score + tracking - density_penalty_score
    }

    pub fn render(&self, camera_x: f32, camera_y: f32) {
        let screen_x = self.x - camera_x;
        let screen_y = self.y - camera_y;
        let current_radius = self.get_current_radius();

        // Viewport culling: only render if cell is visible on screen
        let margin = current_radius * 3.0; // Increased margin for halo effect
        let screen_w = screen_width();
        let screen_h = screen_height();

        if screen_x < -margin
            || screen_x > screen_w + margin
            || screen_y < -margin
            || screen_y > screen_h + margin
        {
            return; // Cell is outside viewport, skip rendering
        }

        // Find nearest dead cell for blob deformation
        let nearest_corpse = self
            .nearest_cells
            .iter()
            .filter(|&&(_, _, _, _, is_alive, _)| is_alive == 0.0)
            .min_by(|&&(_, _, dist_a, _, _, _), &&(_, _, dist_b, _, _, _)| {
                dist_a
                    .partial_cmp(&dist_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

        // Calculate blob deformation parameters
        let deformation_threshold = current_radius * 4.0; // Start deforming at 4x radius distance
        let (should_deform, corpse_angle, deform_strength) =
            if let Some(&(_, angle, distance, _, _, _)) = nearest_corpse {
                if self.state == CellState::Alive && distance < deformation_threshold {
                    let strength = (1.0 - (distance / deformation_threshold)).max(0.0);
                    (true, angle, strength)
                } else {
                    (false, 0.0, 0.0)
                }
            } else {
                (false, 0.0, 0.0)
            };

        // Draw gradient halo effect (only for alive cells and when glow is enabled)
        if self.state == CellState::Alive {
            let halo_layers = 5;
            for i in 0..halo_layers {
                let t = (i as f32) / (halo_layers as f32);
                let halo_radius = current_radius * (1.0 + t * 1.2);
                let alpha = (1.0 - t) * 0.3; // Fade out from 30% to 0%

                let halo_color = Color::new(self.color.r, self.color.g, self.color.b, alpha);

                draw_circle(screen_x, screen_y, halo_radius, halo_color);
            }
        }

        // Draw blob deformation if approaching corpse
        if should_deform {
            let blob_count = 8;
            let extension = current_radius * 0.75 * deform_strength;

            for i in 0..blob_count {
                let angle_offset = (i as f32 / blob_count as f32) * std::f32::consts::TAU;
                let total_angle = self.angle + corpse_angle + angle_offset;

                // Calculate blob position - extends more strongly in the direction of the corpse
                let angle_diff = (angle_offset - corpse_angle).abs();
                let directional_strength = (1.0 - (angle_diff / std::f32::consts::PI)).max(0.0);
                let blob_extension = extension * directional_strength.powf(2.0);

                let blob_x = screen_x + total_angle.cos() * (current_radius + blob_extension * 0.5);
                let blob_y = screen_y + total_angle.sin() * (current_radius + blob_extension * 0.5);
                let blob_radius = current_radius * 0.3 * (1.0 + directional_strength * 0.5);

                let blob_alpha = 0.6 * directional_strength;
                let blob_color = Color::new(self.color.r, self.color.g, self.color.b, blob_alpha);

                draw_circle(blob_x, blob_y, blob_radius, blob_color);
            }
        }

        // Draw the cell body with antialiasing simulation
        if self.state == CellState::Alive {
            // Antialiasing: draw multiple slightly larger circles with decreasing alpha
            for i in 0..3 {
                let aa_radius = current_radius + (i as f32 * 0.5);
                let aa_alpha = if i == 0 { 1.0 } else { 0.3 / (i as f32) };
                let aa_color = Color::new(self.color.r, self.color.g, self.color.b, aa_alpha);
                draw_circle(screen_x, screen_y, aa_radius, aa_color);
            }
        } else {
            // Corpse cells are grayed out with halo effect
            let gray_color = Color::new(
                self.color.r * 0.3,
                self.color.g * 0.3,
                self.color.b * 0.3,
                1.0,
            );

            // Subtle halo for corpses
            for i in 0..3 {
                let t = (i as f32) / 3.0;
                let halo_radius = current_radius * (1.0 + t * 0.5);
                let alpha = (1.0 - t) * 0.15;

                let halo_color = Color::new(gray_color.r, gray_color.g, gray_color.b, alpha);

                draw_circle(screen_x, screen_y, halo_radius, halo_color);
            }

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
        // No energy cost - turning is now rewarded via tracking_score
        self.angle_velocity -= self.turn_rate;
    }

    pub fn turn_right(&mut self) {
        // No energy cost - turning is now rewarded via tracking_score
        self.angle_velocity += self.turn_rate;
    }

    pub fn forward(&mut self) {
        // No energy cost or cooldown - forward movement is now rewarded via tracking_score
        self.velocity_x += self.angle.cos() * self.speed;
        self.velocity_y += self.angle.sin() * self.speed;
    }
}
