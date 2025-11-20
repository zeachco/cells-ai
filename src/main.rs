use macroquad::prelude::*;

fn window_conf() -> Conf {
    Conf {
        window_title: "Cells - Simple Scene".to_owned(),
        fullscreen: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    loop {
        clear_background(DARKBLUE);

        let width = screen_width();
        let height = screen_height();

        // Draw shapes using relative positions based on screen size
        draw_rectangle(width * 0.125, height * 0.167, width * 0.25, height * 0.167, RED);
        draw_circle(width * 0.5, height * 0.5, width * 0.0625, GREEN);
        draw_line(width * 0.0625, height * 0.083, width * 0.9375, height * 0.917, 5.0, WHITE);

        // Additional shapes
        draw_rectangle(width * 0.625, height * 0.667, width * 0.125, height * 0.25, YELLOW);
        draw_circle(width * 0.25, height * 0.75, width * 0.0375, ORANGE);

        next_frame().await
    }
}
