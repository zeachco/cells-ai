use macroquad::prelude::*;

const VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;

varying vec2 uv;

uniform mat4 Model;
uniform mat4 Projection;

void main() {
    gl_Position = Projection * Model * vec4(position, 1.0);
    uv = texcoord;
}
"#;

const FRAGMENT: &str = r#"#version 100
precision highp float;

varying vec2 uv;

uniform vec2 camera_pos;
uniform vec2 resolution;

// Classic sin-based hash — unambiguous in GLSL ES 1.0
float hash2(vec2 p) {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

// Returns brightness of a single star layer.
// world_pos : pixel-space position after parallax offset
// cell_size : size of each hash-cell in pixels
// density   : fraction of cells (0..1) that contain a star
// glow_sigma: Gaussian sigma in normalised cell-space (0..1)
float star_layer(vec2 world_pos, float cell_size, float density, float glow_sigma) {
    vec2 scaled = world_pos / cell_size;
    vec2 cell   = floor(scaled);
    vec2 local  = fract(scaled);

    // Fast reject — most cells are empty
    if (hash2(cell) > density) return 0.0;

    // Deterministic star position within the cell (0.1..0.9 avoids edge clipping)
    float sx   = 0.1 + hash2(cell + vec2(3.7, 0.0)) * 0.8;
    float sy   = 0.1 + hash2(cell + vec2(0.0, 5.3)) * 0.8;
    vec2  star = vec2(sx, sy);

    float dist2      = dot(local - star, local - star);
    float sigma2     = glow_sigma * glow_sigma;
    float brightness = exp(-dist2 / (2.0 * sigma2));

    // Per-star brightness variation
    float dimmer = 0.5 + hash2(cell + vec2(11.1, 7.7)) * 0.5;
    return brightness * dimmer;
}

void main() {
    vec2 screen_pos = uv * resolution;

    // ── Layer 1: Far field — tiny white/cool stars, very slow parallax ──
    vec2  world1 = screen_pos + camera_pos * 0.05;
    float far    = star_layer(world1, 80.0,  0.06, 0.04);
    vec3  col    = vec3(0.75, 0.82, 1.00) * far * 0.9;

    // ── Layer 2: Mid field — blue-white stars, medium parallax ──
    vec2  world2 = screen_pos + camera_pos * 0.15;
    float mid    = star_layer(world2, 150.0, 0.08, 0.05);
    col         += vec3(0.55, 0.70, 1.00) * mid * 1.1;

    // ── Layer 3: Near nebula dust — soft coloured puffs, faster parallax ──
    vec2  world3 = screen_pos + camera_pos * 0.30;
    float neb1   = star_layer(world3,                    300.0, 0.10, 0.10);
    float neb2   = star_layer(world3 + vec2(137.0, 241.0), 280.0, 0.08, 0.12);
    vec3  neb_color = mix(
        vec3(0.38, 0.18, 0.72),   // deep purple
        vec3(0.10, 0.60, 0.65),   // teal
        hash2(floor(world3 / 300.0))
    );
    col += neb_color * (neb1 + neb2) * 0.35;

    gl_FragColor = vec4(clamp(col, 0.0, 1.0), 1.0);
}
"#;

pub struct Background {
    material: Material,
}

impl Background {
    pub fn new() -> Result<Self, macroquad::Error> {
        let material = load_material(
            ShaderSource::Glsl {
                vertex: VERTEX,
                fragment: FRAGMENT,
            },
            MaterialParams {
                uniforms: vec![
                    UniformDesc::new("camera_pos", UniformType::Float2),
                    UniformDesc::new("resolution", UniformType::Float2),
                ],
                ..Default::default()
            },
        )?;
        Ok(Self { material })
    }

    pub fn render(&self, camera_x: f32, camera_y: f32) {
        let w = screen_width();
        let h = screen_height();
        self.material
            .set_uniform("camera_pos", vec2(camera_x, camera_y));
        self.material.set_uniform("resolution", vec2(w, h));
        gl_use_material(&self.material);
        draw_rectangle(0.0, 0.0, w, h, WHITE);
        gl_use_default_material();
    }
}
