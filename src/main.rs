mod background;
mod camera;
mod cell;
mod config;
mod math;
mod neural_network;
mod spatial_grid;
mod stats;
mod storage;
mod world;

use macroquad::prelude::*;
use world::World;

fn window_conf() -> Conf {
    Conf {
        window_title: "Cells - Simple Scene".to_owned(),
        fullscreen: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    // Load custom font
    let font_bytes = include_bytes!("../assets/Inter-Regular.ttf");
    let font = load_ttf_font_from_bytes(font_bytes).ok();

    let mut world = World::spawn(font);

    loop {
        let delta_time = get_frame_time();

        clear_background(BLACK);

        // Handle stats box clicks first
        world.handle_stats_click();

        // Check if mouse is over stats box to skip camera input
        let mouse_pos = mouse_position();
        let skip_camera_mouse = world.is_mouse_over_stats(mouse_pos.0, mouse_pos.1);

        world.camera.handle_input(delta_time, skip_camera_mouse);
        world.camera.update();
        world.update(delta_time);
        world.render();

        next_frame().await
    }
}
