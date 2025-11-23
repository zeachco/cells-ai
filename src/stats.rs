use macroquad::prelude::*;

#[derive(Clone)]
pub struct BestCellStats {
    pub total_energy_accumulated: f32,
    pub current_energy: f32,
    pub children_count: usize,
    pub color: Color,
    pub age: f32,
    pub x: f32,
    pub y: f32,
    pub is_alive: bool,
}

pub struct Stats {
    best_cell: Option<BestCellStats>,
    selected: bool,
}

impl Stats {
    pub fn new() -> Self {
        Stats {
            best_cell: None,
            selected: true, // Start selected by default
        }
    }

    // Clear the stats (remove the tracked cell)
    pub fn clear(&mut self) {
        self.best_cell = None;
    }

    // Directly set the best cell without fitness comparison (shows current best alive cell)
    pub fn set(&mut self, stats: BestCellStats) {
        self.best_cell = Some(stats);
    }

    // Calculate a fitness score for a cell based on energy accumulated and children count
    pub fn calculate_fitness(energy: f32, children: usize) -> f32 {
        // Weight both energy and children equally
        // Normalize children to be on a similar scale as energy
        energy + (children as f32 * 100.0)
    }

    // Get the bounds of the stats box for click detection
    fn get_bounds(&self) -> Option<(f32, f32, f32, f32)> {
        self.best_cell.as_ref()?;

        let screen_w = screen_width();
        let font_size = 24.0;
        let line_height = 30.0;
        let padding = 20.0;

        let title = "Best Cell:";
        let line1 = "Energy: 999999.9"; // Max width estimate
        let line2 = "Children: 99999";
        let line3 = "Age: 999.9";
        let line4 = "Fitness: 999999.9";

        let max_width = [
            measure_text(title, None, font_size as u16, 1.0).width,
            measure_text(line1, None, font_size as u16, 1.0).width,
            measure_text(line2, None, font_size as u16, 1.0).width,
            measure_text(line3, None, font_size as u16, 1.0).width,
            measure_text(line4, None, font_size as u16, 1.0).width,
        ]
        .iter()
        .cloned()
        .fold(0.0_f32, f32::max);

        let bg_padding = 10.0;
        let bg_x = screen_w - max_width - padding - bg_padding * 2.0;
        let bg_y = padding - bg_padding;
        let bg_width = max_width + bg_padding * 2.0;
        let bg_height = line_height * 5.0 + bg_padding;

        Some((bg_x, bg_y, bg_width, bg_height))
    }

    // Check if mouse position is over the stats box
    pub fn is_mouse_over(&self, mouse_x: f32, mouse_y: f32) -> bool {
        if let Some((x, y, w, h)) = self.get_bounds() {
            mouse_x >= x && mouse_x <= x + w && mouse_y >= y && mouse_y <= y + h
        } else {
            false
        }
    }

    // Toggle selection state
    pub fn toggle_selection(&mut self) {
        self.selected = !self.selected;
    }

    // Check if stats box is selected
    pub fn is_selected(&self) -> bool {
        self.selected
    }

    // Get the selected cell's position if selected and alive
    pub fn get_selected_position(&self) -> Option<(f32, f32)> {
        if self.selected {
            self.best_cell.as_ref().and_then(|cell| {
                if cell.is_alive {
                    Some((cell.x, cell.y))
                } else {
                    None
                }
            })
        } else {
            None
        }
    }

    // Render stats in top-right corner
    pub fn render(&self) {
        if let Some(best) = &self.best_cell {
            let screen_w = screen_width();
            let font_size = 24.0;
            let line_height = 30.0;
            let padding = 20.0;

            // Calculate text widths (approximate)
            let title = "Best Cell:";
            let line1 = format!("Energy: {:.1}", best.current_energy);
            let line2 = format!("Children: {}", best.children_count);
            let line3 = format!("Age: {:.1}", best.age);
            let fitness =
                Self::calculate_fitness(best.total_energy_accumulated, best.children_count);
            let line4 = format!("Fitness: {:.1}", fitness);

            // Find the longest line for background width
            let max_width = [
                measure_text(title, None, font_size as u16, 1.0).width,
                measure_text(&line1, None, font_size as u16, 1.0).width,
                measure_text(&line2, None, font_size as u16, 1.0).width,
                measure_text(&line3, None, font_size as u16, 1.0).width,
                measure_text(&line4, None, font_size as u16, 1.0).width,
            ]
            .iter()
            .cloned()
            .fold(0.0_f32, f32::max);

            // Draw semi-transparent background
            let bg_padding = 10.0;
            let bg_x = screen_w - max_width - padding - bg_padding * 2.0;
            let bg_y = padding - bg_padding;
            let bg_width = max_width + bg_padding * 2.0;
            let bg_height = line_height * 5.0 + bg_padding;

            draw_rectangle(
                bg_x,
                bg_y,
                bg_width,
                bg_height,
                Color::new(0.0, 0.0, 0.0, 0.7),
            );

            // Draw highlighted border if selected
            if self.selected {
                let border_thickness = 3.0;
                draw_rectangle_lines(
                    bg_x,
                    bg_y,
                    bg_width,
                    bg_height,
                    border_thickness,
                    best.color,
                );
            }

            // Draw title with status indicator
            let x = screen_w - max_width - padding;
            let title_with_status = if best.is_alive {
                "Best Cell:"
            } else {
                "Best Cell: (DEAD)"
            };
            draw_text(title_with_status, x, padding + font_size, font_size, WHITE);

            // Draw color indicator
            let color_y = padding + font_size + line_height / 2.0;
            draw_circle(x + 10.0, color_y, 8.0, best.color);

            // Draw stats
            draw_text(
                &line1,
                x,
                padding + font_size + line_height,
                font_size,
                WHITE,
            );
            draw_text(
                &line2,
                x,
                padding + font_size + line_height * 2.0,
                font_size,
                WHITE,
            );
            draw_text(
                &line3,
                x,
                padding + font_size + line_height * 3.0,
                font_size,
                WHITE,
            );
            draw_text(
                &line4,
                x,
                padding + font_size + line_height * 4.0,
                font_size,
                WHITE,
            );
        }
    }
}
