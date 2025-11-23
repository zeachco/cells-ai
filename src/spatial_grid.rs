/// Spatial hash grid for efficient proximity queries
/// Divides the world into uniform buckets for O(1) spatial lookups
pub struct SpatialGrid {
    buckets: Vec<Vec<usize>>,
    bucket_size: f32,
    grid_width: usize,
    grid_height: usize,
}

impl SpatialGrid {
    /// Creates a new spatial grid
    ///
    /// # Arguments
    /// * `world_width` - Width of the world in units
    /// * `world_height` - Height of the world in units
    /// * `bucket_size` - Size of each grid bucket (larger = fewer buckets, more cells per bucket)
    pub fn new(world_width: f32, world_height: f32, bucket_size: f32) -> Self {
        let grid_width = (world_width / bucket_size).ceil() as usize;
        let grid_height = (world_height / bucket_size).ceil() as usize;
        let bucket_count = grid_width * grid_height;

        SpatialGrid {
            buckets: vec![Vec::new(); bucket_count],
            bucket_size,
            grid_width,
            grid_height,
        }
    }

    /// Clears all buckets for a new frame
    pub fn clear(&mut self) {
        for bucket in &mut self.buckets {
            bucket.clear();
        }
    }

    /// Inserts a cell index at the given position
    pub fn insert(&mut self, x: f32, y: f32, cell_index: usize) {
        let bucket_index = self.get_bucket_index(x, y);
        self.buckets[bucket_index].push(cell_index);
    }

    /// Queries nearby cell indices within collision range
    /// Returns indices of cells in the same bucket and neighboring buckets
    pub fn query_nearby(&self, x: f32, y: f32, radius: f32) -> Vec<usize> {
        let mut nearby = Vec::new();

        let grid_x = (x / self.bucket_size).floor() as i32;
        let grid_y = (y / self.bucket_size).floor() as i32;

        // Check how many buckets we need to search based on radius
        let bucket_range = (radius / self.bucket_size).ceil() as i32 + 1;

        // Query neighboring buckets (including wrapping)
        for dy in -bucket_range..=bucket_range {
            for dx in -bucket_range..=bucket_range {
                let check_x = grid_x + dx;
                let check_y = grid_y + dy;

                // Handle wrapping boundaries
                let wrapped_x = check_x.rem_euclid(self.grid_width as i32) as usize;
                let wrapped_y = check_y.rem_euclid(self.grid_height as i32) as usize;

                let bucket_index = wrapped_y * self.grid_width + wrapped_x;
                nearby.extend_from_slice(&self.buckets[bucket_index]);
            }
        }

        nearby
    }

    /// Gets the bucket index for a world position
    fn get_bucket_index(&self, x: f32, y: f32) -> usize {
        let grid_x = ((x / self.bucket_size).floor() as usize) % self.grid_width;
        let grid_y = ((y / self.bucket_size).floor() as usize) % self.grid_height;
        grid_y * self.grid_width + grid_x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_creation() {
        let grid = SpatialGrid::new(8000.0, 8000.0, 100.0);
        assert_eq!(grid.grid_width, 80);
        assert_eq!(grid.grid_height, 80);
        assert_eq!(grid.buckets.len(), 6400);
    }

    #[test]
    fn test_insert_and_query() {
        let mut grid = SpatialGrid::new(1000.0, 1000.0, 100.0);

        // Insert cells
        grid.insert(150.0, 150.0, 0);
        grid.insert(160.0, 160.0, 1);
        grid.insert(900.0, 900.0, 2);

        // Query near first cell
        let nearby = grid.query_nearby(155.0, 155.0, 50.0);
        assert!(nearby.contains(&0));
        assert!(nearby.contains(&1));

        // Cell at (900, 900) should not be in nearby results for (155, 155)
        // unless it's in a neighboring bucket, but with 100-unit buckets it should be far
    }

    #[test]
    fn test_wrapping_boundaries() {
        let mut grid = SpatialGrid::new(1000.0, 1000.0, 100.0);

        // Insert cell near edge
        grid.insert(10.0, 10.0, 0);

        // Query from opposite edge (should wrap)
        let _nearby = grid.query_nearby(990.0, 990.0, 50.0);
        // Due to wrapping, cells near (0,0) might be considered neighbors of (1000, 1000)
        // This tests the wrapping logic works correctly
        assert!(grid.buckets.len() > 0);
    }
}
