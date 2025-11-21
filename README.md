# Cells

**Live Demo:** [https://zeachco.github.io/cells-ai/](https://zeachco.github.io/cells-ai/)

A Rust-based cellular simulation that compiles to WebAssembly and runs in the browser.

## Project Structure

This is a **Rust project** built with [macroquad](https://github.com/not-fl3/macroquad) that compiles to WebAssembly for web deployment. While the core logic is written in Rust, the project uses npm scripts for convenient web development workflow.

## Prerequisites

- [Rust](https://rustup.rs/) (with the `wasm32-unknown-unknown` target)
- [Node.js](https://nodejs.org/) and npm
- [cargo-watch](https://crates.io/crates/cargo-watch) (optional, for development)

### Setting up Rust for WebAssembly

```bash
# Add the WebAssembly target
rustup target add wasm32-unknown-unknown

# Install cargo-watch for development (optional)
cargo install cargo-watch
```

## Getting Started

### Installation

```bash
npm install
```

### Development

Start the development server with hot-reloading:

```bash
npm run dev
```

This runs both:
- A Rust watcher that rebuilds the WebAssembly on code changes
- A Vite dev server at `http://localhost:5173`

### Building for Production

```bash
npm run build
```

This compiles the Rust code to optimized WebAssembly and bundles everything with Vite.

### Preview Production Build

```bash
npm run preview
```

## Available Scripts

| Command | Description |
|---------|-------------|
| `npm run dev` | Start development server with hot-reloading |
| `npm run build` | Build optimized production bundle |
| `npm run build:wasm` | Compile Rust to WebAssembly (release mode) |
| `npm run watch:wasm` | Watch Rust files and rebuild on changes |
| `npm run preview` | Preview the production build locally |

## Rust Development

While npm provides convenient scripts, you can also use standard Rust/Cargo commands:

```bash
# Build for native (desktop)
cargo build

# Run natively
cargo run

# Build for WebAssembly
cargo build --release --target wasm32-unknown-unknown
```

## Repository

[github.com/zeachco/cells-ai](https://github.com/zeachco/cells-ai)
