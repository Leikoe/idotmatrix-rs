use crate::color::Color;
use crate::frame::{Frame, MatrixSize};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct Life {
    size: MatrixSize,
    cells: Vec<bool>,
    ages: Vec<u8>,
    wrap: bool,
    seen: HashSet<Vec<bool>>,
}

impl Life {
    pub fn random(size: MatrixSize, density: f32, seed: Option<u64>, wrap: bool) -> Self {
        let mut rng = match seed {
            Some(seed) => StdRng::seed_from_u64(seed),
            None => StdRng::from_entropy(),
        };
        let cells = (0..size.pixels())
            .map(|_| rng.r#gen::<f32>() < density)
            .collect();
        Self {
            size,
            cells,
            ages: vec![0; size.pixels()],
            wrap,
            seen: HashSet::new(),
        }
    }

    pub fn pattern(size: MatrixSize, name: &str, wrap: bool) -> Self {
        let points: &[(usize, usize)] = match name {
            "glider" => &[(1, 0), (2, 1), (0, 2), (1, 2), (2, 2)],
            "rpentomino" => &[(1, 0), (2, 0), (0, 1), (1, 1), (1, 2)],
            "acorn" => &[(1, 0), (3, 1), (0, 2), (1, 2), (4, 2), (5, 2), (6, 2)],
            "diehard" => &[(6, 0), (0, 1), (1, 1), (1, 2), (5, 2), (6, 2), (7, 2)],
            _ => &[],
        };
        let max_x = points.iter().map(|(x, _)| *x).max().unwrap_or(0);
        let max_y = points.iter().map(|(_, y)| *y).max().unwrap_or(0);
        let ox = size.width.saturating_sub(max_x + 1) / 2;
        let oy = size.height.saturating_sub(max_y + 1) / 2;
        let mut cells = vec![false; size.pixels()];
        for (x, y) in points {
            let x = x + ox;
            let y = y + oy;
            if x < size.width && y < size.height {
                cells[y * size.width + x] = true;
            }
        }
        Self {
            size,
            cells,
            ages: vec![0; size.pixels()],
            wrap,
            seen: HashSet::new(),
        }
    }

    pub fn step(&mut self) {
        let mut next = vec![false; self.cells.len()];
        for y in 0..self.size.height {
            for x in 0..self.size.width {
                let count = self.neighbor_count(x, y);
                let idx = self.index(x, y);
                next[idx] = count == 3 || (self.cells[idx] && count == 2);
            }
        }
        self.cells = next;
    }

    pub fn reset_if_stale(&mut self, density: f32) {
        let alive = self.alive_count();
        if alive == 0 || !self.seen.insert(self.cells.clone()) {
            *self = Life::random(self.size, density, None, self.wrap);
        }
    }

    pub fn frame(&mut self, color: Color, background: Color, trail_decay: u8) -> Frame {
        Frame::from_fn(self.size, |x, y| {
            let idx = self.index(x, y);
            if self.cells[idx] {
                self.ages[idx] = 255;
            } else {
                self.ages[idx] = self.ages[idx].saturating_sub(trail_decay);
            }
            background.lerp(color, self.ages[idx] as f32 / 255.0)
        })
    }

    pub fn alive_count(&self) -> usize {
        self.cells.iter().filter(|cell| **cell).count()
    }

    fn neighbor_count(&self, x: usize, y: usize) -> u8 {
        let mut count = 0;
        for dy in [-1isize, 0, 1] {
            for dx in [-1isize, 0, 1] {
                if dx == 0 && dy == 0 {
                    continue;
                }
                if let Some(idx) = self.neighbor_index(x, y, dx, dy) {
                    count += self.cells[idx] as u8;
                }
            }
        }
        count
    }

    fn neighbor_index(&self, x: usize, y: usize, dx: isize, dy: isize) -> Option<usize> {
        let nx = x as isize + dx;
        let ny = y as isize + dy;
        if self.wrap {
            let x = nx.rem_euclid(self.size.width as isize) as usize;
            let y = ny.rem_euclid(self.size.height as isize) as usize;
            Some(self.index(x, y))
        } else if nx >= 0
            && ny >= 0
            && (nx as usize) < self.size.width
            && (ny as usize) < self.size.height
        {
            Some(self.index(nx as usize, ny as usize))
        } else {
            None
        }
    }

    fn index(&self, x: usize, y: usize) -> usize {
        y * self.size.width + x
    }
}
