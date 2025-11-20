mod camera;
mod cell;
mod math;
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
    let mut world = World::spawn();

    loop {
        let delta_time = get_frame_time();

        clear_background(DARKBLUE);

        world.camera.handle_input(delta_time);
        world.camera.update();
        world.update();
        world.render();

        next_frame().await
    }
}
