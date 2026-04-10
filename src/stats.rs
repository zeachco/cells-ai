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
    pub brain_tier: usize,
    pub brain_operations: usize,
    pub cell_index: usize,
    pub prev_best_score: f32, // Previous best score for this tier
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
        let screen_h = screen_height();
        let font_size = 23.0;
        let line_height = 30.0;
        let padding = 20.0;

        let title = "Best Cell:";
        let line1 = "Index: 99999"; // Max width estimate
        let line2 = "Energy: 999999.9";
        let line3 = "Children: 99999";
        let line4 = "Generation: 99999";
        let line5 = "Brain: m3 (99999 operations)";
        let line6 = "Age: 999.9";
        let line7 = "Score: 999999.9 + 999999.9"; // Account for diff display
        let line8 = "Pos: (9999.9, 9999.9)";

        let max_width = [
            measure_text(title, font, font_size as u16, 1.0).width,
            measure_text(line1, font, font_size as u16, 1.0).width,
            measure_text(line2, font, font_size as u16, 1.0).width,
            measure_text(line3, font, font_size as u16, 1.0).width,
            measure_text(line4, font, font_size as u16, 1.0).width,
            measure_text(line5, font, font_size as u16, 1.0).width,
            measure_text(line6, font, font_size as u16, 1.0).width,
            measure_text(line7, font, font_size as u16, 1.0).width,
            measure_text(line8, font, font_size as u16, 1.0).width,
        ]
        .iter()
        .cloned()
        .fold(0.0_f32, f32::max);

        let bg_padding = 10.0;
        let bg_x = screen_w - max_width - padding - bg_padding * 2.0;
        let bg_width = max_width + bg_padding * 2.0;
        let bg_height = line_height * 9.0 + bg_padding;
        let bg_y = screen_h - bg_height - padding;

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

    // Render stats in bottom-right corner
    pub fn render(&self, font: Option<&Font>) {
        if let Some(best) = &self.best_cell {
            let screen_w = screen_width();
            let screen_h = screen_height();
            let font_size = 23.0;
            let line_height = 30.0;
            let padding = 30.0;

            let score =
                Self::calculate_score(best.children_count, best.energy_from_cells, best.age);

            // Calculate text widths (approximate)
            let title = "Best Cell:";
            let line1 = format!("Index: {}", best.cell_index);
            let line2 = format!("Energy: {:.1}", best.current_energy);
            let line3 = format!("Children: {}", best.children_count);
            let line4 = format!("Generation: {}", best.generation);
            let line5 = format!(
                "Brain: m{} ({} operations)",
                best.brain_tier, best.brain_operations
            );
            let line6 = format!("Age: {:.1}", best.age);

            // Check if current score beats previous best
            let score_diff = score - best.prev_best_score;
            let line7 = if score > best.prev_best_score {
                format!("Score: {:.1}", best.prev_best_score)
            } else {
                format!("Score: {:.1}", score)
            };
            let line7_diff = if score > best.prev_best_score {
                format!(" + {:.1}", score_diff)
            } else {
                String::new()
            };

            let line8 = format!("Pos: ({:.1}, {:.1})", best.x, best.y);

            // Find the longest line for background width
            let line7_full = format!("{}{}", line7, line7_diff);
            let max_width = [
                measure_text(title, font, font_size as u16, 1.0).width,
                measure_text(&line1, font, font_size as u16, 1.0).width,
                measure_text(&line2, font, font_size as u16, 1.0).width,
                measure_text(&line3, font, font_size as u16, 1.0).width,
                measure_text(&line4, font, font_size as u16, 1.0).width,
                measure_text(&line5, font, font_size as u16, 1.0).width,
                measure_text(&line6, font, font_size as u16, 1.0).width,
                measure_text(&line7_full, font, font_size as u16, 1.0).width,
                measure_text(&line8, font, font_size as u16, 1.0).width,
            ]
            .iter()
            .cloned()
            .fold(0.0_f32, f32::max);

            // Draw semi-transparent background
            let bg_padding = 10.0;
            let bg_x = screen_w - max_width - padding - bg_padding * 2.0;
            let bg_width = max_width + bg_padding * 2.0;
            let bg_height = line_height * 9.0 + bg_padding;
            let bg_y = screen_h - bg_height - padding;

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
                bg_y + bg_padding + font_size,
                text_params.clone(),
            );

            draw_text_ex(
                &line1,
                x,
                bg_y + bg_padding + font_size + line_height,
                text_params.clone(),
            );
            draw_text_ex(
                &line2,
                x,
                bg_y + bg_padding + font_size + line_height * 2.0,
                text_params.clone(),
            );
            draw_text_ex(
                &line3,
                x,
                bg_y + bg_padding + font_size + line_height * 3.0,
                text_params.clone(),
            );
            draw_text_ex(
                &line4,
                x,
                bg_y + bg_padding + font_size + line_height * 4.0,
                text_params.clone(),
            );
            draw_text_ex(
                &line5,
                x,
                bg_y + bg_padding + font_size + line_height * 5.0,
                text_params.clone(),
            );
            draw_text_ex(
                &line6,
                x,
                bg_y + bg_padding + font_size + line_height * 6.0,
                text_params.clone(),
            );

            // Draw score line (base score in white)
            let score_y = bg_y + bg_padding + font_size + line_height * 7.0;
            draw_text_ex(&line7, x, score_y, text_params.clone());

            // If beating previous best, draw the diff in green
            if !line7_diff.is_empty() {
                let line7_width = measure_text(&line7, font, font_size as u16, 1.0).width;
                draw_text_ex(
                    &line7_diff,
                    x + line7_width,
                    score_y,
                    TextParams {
                        font,
                        font_size: font_size as u16,
                        color: Color::new(0.0, 1.0, 0.0, 1.0), // Bright green
                        ..Default::default()
                    },
                );
            }

            draw_text_ex(
                &line8,
                x,
                bg_y + bg_padding + font_size + line_height * 8.0,
                text_params,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_score_zeros() {
        let score = Stats::calculate_score(0, 0.0, 0.0);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_calculate_score_children() {
        let score = Stats::calculate_score(2, 0.0, 0.0);
        assert_eq!(score, 200.0);
    }

    #[test]
    fn test_calculate_score_energy() {
        let score = Stats::calculate_score(0, 50.5, 0.0);
        assert_eq!(score, 50.5);
    }

    #[test]
    fn test_calculate_score_age() {
        let score = Stats::calculate_score(0, 0.0, 10.0);
        assert_eq!(score, 100.0);
    }

    #[test]
    fn test_calculate_score_combination() {
        let score = Stats::calculate_score(1, 20.0, 5.0);
        assert_eq!(score, 170.0); // 100 + 20 + 50
    }

    #[test]
    fn test_calculate_score_large_values() {
        let score = Stats::calculate_score(1000, 10000.0, 1000.0);
        // 1000 * 100 + 10000 + 1000 * 10 = 100000 + 10000 + 10000 = 120000
        assert_eq!(score, 120000.0);
    }
}
