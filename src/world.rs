use crate::camera::Camera;
use crate::cell::Cell;

pub struct World {
    pub cells: Vec<Cell>,
    pub camera: Camera,
}

impl World {
    pub fn spawn() -> Self {
        let mut cells = Vec::new();
        for _ in 0..1000 {
            cells.push(Cell::spawn());
        }

        World {
            cells,
            camera: Camera::new(),
        }
    }

    pub fn update(&mut self) {
        for cell in &mut self.cells {
            cell.update();
        }
    }

    pub fn render(&self) {
        for cell in &self.cells {
            cell.render(self.camera.x, self.camera.y);
        }
    }
}
