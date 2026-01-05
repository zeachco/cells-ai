use macroquad::prelude::*;

#[derive(Clone)]
pub struct BestCellStats {
    pub energy_from_cells: f32,
    pub current_energy: f32,
    pub children_count: usize,
    pub generation: usize,
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

    // Calculate comprehensive score based on children, energy from cells, and age
    pub fn calculate_score(children: usize, energy_from_cells: f32, age: f32) -> f32 {
        // Children count: 100 points per child (primary metric)
        let children_score = children as f32 * 100.0;

        // Energy from cells: 1 point per energy (equally important as children)
        let energy_score = energy_from_cells;

        // Age: 10 points per age unit (secondary metric - older cells have survived longer)
        let age_score = age * 10.0;

        children_score + energy_score + age_score
    }

    // Get the bounds of the stats box for click detection
    fn get_bounds(&self, font: Option<&Font>) -> Option<(f32, f32, f32, f32)> {
        self.best_cell.as_ref()?;

        let screen_w = screen_width();
        let font_size = 24.0;
        let line_height = 30.0;
        let padding = 20.0;

        let title = "Best Cell:";
        let line1 = "Energy: 999999.9"; // Max width estimate
        let line2 = "Children: 99999";
        let line3 = "Generation: 99999";
        let line4 = "Age: 999.9";
        let line5 = "Score: 999999.9";
        let line6 = "Pos: (9999.9, 9999.9)";

        let max_width = [
            measure_text(title, font, font_size as u16, 1.0).width,
            measure_text(line1, font, font_size as u16, 1.0).width,
            measure_text(line2, font, font_size as u16, 1.0).width,
            measure_text(line3, font, font_size as u16, 1.0).width,
            measure_text(line4, font, font_size as u16, 1.0).width,
            measure_text(line5, font, font_size as u16, 1.0).width,
            measure_text(line6, font, font_size as u16, 1.0).width,
        ]
        .iter()
        .cloned()
        .fold(0.0_f32, f32::max);

        let bg_padding = 10.0;
        let bg_x = screen_w - max_width - padding - bg_padding * 2.0;
        let bg_y = padding - bg_padding;
        let bg_width = max_width + bg_padding * 2.0;
        let bg_height = line_height * 7.0 + bg_padding;

        Some((bg_x, bg_y, bg_width, bg_height))
    }

    // Check if mouse position is over the stats box
    pub fn is_mouse_over(&self, mouse_x: f32, mouse_y: f32, font: Option<&Font>) -> bool {
        if let Some((x, y, w, h)) = self.get_bounds(font) {
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
    pub fn render(&self, font: Option<&Font>) {
        if let Some(best) = &self.best_cell {
            let screen_w = screen_width();
            let font_size = 24.0;
            let line_height = 30.0;
            let padding = 30.0;

            let score =
                Self::calculate_score(best.children_count, best.energy_from_cells, best.age);

            // Calculate text widths (approximate)
            let title = "Best Cell:";
            let line1 = format!("Energy: {:.1}", best.current_energy);
            let line2 = format!("Children: {}", best.children_count);
            let line3 = format!("Generation: {}", best.generation);
            let line4 = format!("Age: {:.1}", best.age);
            let line5 = format!("Score: {:.1}", score);
            let line6 = format!("Pos: ({:.1}, {:.1})", best.x, best.y);

            // Find the longest line for background width
            let max_width = [
                measure_text(title, font, font_size as u16, 1.0).width,
                measure_text(&line1, font, font_size as u16, 1.0).width,
                measure_text(&line2, font, font_size as u16, 1.0).width,
                measure_text(&line3, font, font_size as u16, 1.0).width,
                measure_text(&line4, font, font_size as u16, 1.0).width,
                measure_text(&line5, font, font_size as u16, 1.0).width,
                measure_text(&line6, font, font_size as u16, 1.0).width,
            ]
            .iter()
            .cloned()
            .fold(0.0_f32, f32::max);

            // Draw semi-transparent background
            let bg_padding = 10.0;
            let bg_x = screen_w - max_width - padding - bg_padding * 2.0;
            let bg_y = padding - bg_padding;
            let bg_width = max_width + bg_padding * 2.0;
            let bg_height = line_height * 7.0 + bg_padding;

            draw_rectangle(
                bg_x,
                bg_y,
                bg_width + bg_padding,
                bg_height + bg_padding,
                Color::new(0.0, 0.0, 0.0, 0.8),
            );

            // Draw highlighted border if selected
            if self.selected {
                let border_thickness = 2.0;
                draw_rectangle_lines(
                    bg_x,
                    bg_y,
                    bg_width + bg_padding,
                    bg_height + bg_padding,
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

            let text_params = TextParams {
                font,
                font_size: font_size as u16,
                color: WHITE,
                ..Default::default()
            };

            draw_text_ex(
                title_with_status,
                x,
                padding + font_size,
                text_params.clone(),
            );

            draw_text_ex(
                &line1,
                x,
                padding + font_size + line_height,
                text_params.clone(),
            );
            draw_text_ex(
                &line2,
                x,
                padding + font_size + line_height * 2.0,
                text_params.clone(),
            );
            draw_text_ex(
                &line3,
                x,
                padding + font_size + line_height * 3.0,
                text_params.clone(),
            );
            draw_text_ex(
                &line4,
                x,
                padding + font_size + line_height * 4.0,
                text_params.clone(),
            );
            draw_text_ex(
                &line5,
                x,
                padding + font_size + line_height * 5.0,
                text_params.clone(),
            );
            draw_text_ex(
                &line6,
                x,
                padding + font_size + line_height * 6.0,
                text_params,
            );
        }
    }
}
