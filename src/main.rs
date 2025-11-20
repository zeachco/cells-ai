use macroquad::prelude::*;

fn window_conf() -> Conf {
    Conf {
        window_title: "Cells - Simple Scene".to_owned(),
        window_width: 800,
        window_height: 600,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    loop {
        clear_background(DARKBLUE);

        // Draw some basic shapes to create a simple scene
        draw_rectangle(100.0, 100.0, 200.0, 100.0, RED);
        draw_circle(400.0, 300.0, 50.0, GREEN);
        draw_line(50.0, 50.0, 750.0, 550.0, 5.0, WHITE);

        // Draw some additional shapes for a more interesting scene
        draw_rectangle(500.0, 400.0, 100.0, 150.0, YELLOW);
        draw_circle(200.0, 450.0, 30.0, ORANGE);

        next_frame().await
    }
}
