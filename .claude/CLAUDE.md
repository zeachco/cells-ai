# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A Rust-based cellular evolution simulation using neural networks, compiled to WebAssembly for browser deployment. Cells evolve through natural selection, developing behaviors via neural network "brains" that mutate across generations.

## Build and Development Commands

### Development
```bash
# Start development server with hot-reloading (both Rust and web)
npm run dev

# Development server runs on http://localhost:3000
# Vite watches for WASM changes and triggers full page reload
```

### Building
```bash
# Build WASM only (release mode for wasm32-unknown-unknown target)
npm run build:wasm

# Full production build (WASM + web bundle)
npm run build

# Preview production build
npm run preview
```

### Native Rust Development
```bash
# Run natively (macroquad supports both native and WASM)
cargo run

# Build for native target
cargo build

# Run tests
cargo test
```

### Code Quality
Pre-commit hooks via lefthook automatically run:
- `cargo fmt --all --check` - Format checking
- `cargo clippy --target wasm32-unknown-unknown -- -D warnings` - Linting (fails on warnings)
- `cargo check` - Compilation check

## Architecture

### Core Simulation Loop (src/main.rs)
1. World updates (if not paused)
2. Camera input handling (with UI collision detection)
3. Rendering (cells, sensors, stats, boundaries)

### Key Systems

#### World Simulation (src/world.rs)
- **Spatial Grid Optimization**: Uses `SpatialGrid` (100-unit buckets) for O(1) proximity queries instead of O(n²) collision checks
- **Parallel Processing**: Rayon parallelizes cell updates, collision detection, and sensor updates
- **Adaptive Performance**: Dynamically adjusts `max_cells` cap based on FPS (target: 30-240 FPS)
- **Genome Preservation**: Stores best cell genome for respawning after extinction
- **Diversity Tracking**: Calculates color (hue) variance to track genetic diversity

**Simulation Controls**:
- `SPACE`: Toggle pause
- `R`: Reset world with best genome
- `+/-`: Adjust simulation speed (0.125x to 8.0x)
- `1`: Reset to normal speed

#### Cell Behavior (src/cell.rs)
Each cell has:
- **Individual State**: Position, energy, velocity, age (affects size and energy costs)
- **Inherited Attributes**: Color, radius, speed, turn rate, energy chunk size, species multiplier, mass (max energy capacity)
- **Neural Network Brain**: 20 inputs (5 sensors × 4 values), 4 outputs (actions)
- **Stats Tracking**: Total energy accumulated, children count (used for fitness calculation)

**Energy System**:
- Metabolism drains energy each tick
- Age increases costs (1x to 2x multiplier)
- Young cells (age < 20) burn all gained energy for growth
- Reproduction at >100 energy: 2/3 to child, 1/3 to parent
- Population capped at `max_cells` (dynamic based on FPS)

**Sensors**: Each sensor tracks nearest cells within 200 units:
- Angle from facing direction (-180° to 180°)
- Distance (0-200 units)
- Target mass (max energy capacity)
- Is alive (1.0 = alive, 0.0 = corpse)

Sensors prioritize: dead cells > high energy > close proximity

#### Neural Network (src/neural_network.rs)
- **Architecture**: Input → Hidden (ReLU) → Output
- Hidden layer size: `2 * (inputs + outputs)` = 48 nodes
- **Mutation**: 1-10% mutation rate on reproduction, adjusts weights by ±0.1, clamped to [-2.0, 2.0]
- **Actions**: 0=no-op, 1=turn_left, 2=turn_right, 3=forward
- Decision made each frame via `get_best_action()` (argmax of outputs)

#### Spatial Grid (src/spatial_grid.rs)
Hash grid partitions world into 100-unit buckets for efficient proximity queries.
- Handles world wrapping at boundaries
- Query returns cells in neighboring buckets within radius
- Reduces collision/sensor checks from O(n²) to O(k) where k = cells in nearby buckets

#### Camera System (src/camera.rs)
- WASD: Pan camera
- Q/E: Rotate (currently unused in rendering)
- Mouse drag: Direct camera movement with momentum on release
- Trackpad/scroll wheel: Natural scrolling with momentum
- Auto-follow: Clicking stats box enables camera tracking of best cell

#### Stats Display (src/stats.rs)
Top-right corner shows best living cell:
- Current energy, children count, age
- Fitness score: `total_energy_accumulated + (children_count * 100)`
- Click to toggle camera follow (highlighted border when selected)
- Color indicator shows cell's evolved hue

## Important Implementation Details

### Mutations
All inherited attributes mutate by ±1% during reproduction:
- Numeric traits: clamped to their spawn ranges
- Color (hue): wraps around 360° spectrum
- Neural network: 1-10% of weights/biases adjusted by ±0.1

### Performance Optimizations
1. **Parallel Processing**: All cell updates, collisions, and sensor updates use Rayon
2. **Spatial Partitioning**: Spatial grid reduces O(n²) to O(k) for proximity queries
3. **Viewport Culling**: Only renders cells visible on screen
4. **Adaptive Population**: FPS-based dynamic cell cap (adjusts every 2 seconds)
5. **Partial Sorting**: Uses `select_nth_unstable_by` for sensor prioritization instead of full sort

### World Wrapping
World boundaries wrap (toroidal topology):
- Cells leaving one edge appear on opposite edge
- Distance calculations account for wrapping
- Spatial grid handles wrapped neighbor queries

### Fitness Function
`total_energy_accumulated + (children_count * 100)`

Balances energy collection with reproductive success. Used to identify best genome for preservation.

## Testing
Tests exist for:
- Neural network creation, forward pass, mutation, action selection
- Spatial grid creation, insertion, queries, wrapping

Run with `cargo test` (note: tests run on native target, not WASM)

## WebAssembly Build
- Target: `wasm32-unknown-unknown`
- Release profile: `opt-level = 3`, `lto = true`, `codegen-units = 1`
- Vite plugin watches WASM file and triggers full reload on rebuild
- WASM file copied to `dist/` during production build
