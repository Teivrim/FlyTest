//! The course: a maze rebuilt every round, with the food at the far end.
//!
//! The acts used to hold a fixed ring of posts and a moving target point, and a
//! character that touched the point scored a point. Nothing about that asked
//! anything of her: the same walk solved it every time, the scenery never
//! changed, and there was nothing to learn and nothing to watch happen.
//!
//! A course is a real maze on a grid, inscribed in the stage's disc, with the
//! food at one end and the characters set down at the other. It is built from a
//! seed, so a round can be repeated exactly, but a new round is a new maze: a
//! route learned in the last one is worthless in this one, which is the point.
//!
//! Everything the client draws comes from here, and the collision circles are
//! derived from the same wall segments by [`Course::solids`], so the wall
//! somebody sees and the wall somebody bumps into cannot be two objects.

/// A wall, as a slab centred on a point with its long axis at an angle.
///
/// Oriented rather than round, because a maze is made of straight runs and a
/// circle cannot say "this is a wall, that way along".
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wall {
    /// Centre, in stage-local coordinates.
    pub x: f32,
    pub z: f32,
    /// Half the length, along the wall.
    pub half_len: f32,
    /// Half the thickness, across it.
    pub half_thick: f32,
    /// Direction of the wall in radians, measured from the x axis.
    pub angle: f32,
}

/// A course: a maze, the way in, the food, and the way through.
///
/// The solution is kept because it is the only honest way to say how a character
/// is doing, and it is deliberately never given to the steering. Handed the
/// answer, she walks it in a straight line and looks like a cursor. She has to
/// find it, so she is handed a smell and a wall.
#[derive(Debug, Clone)]
pub struct Course {
    /// The seed this maze was built from.
    pub seed: u32,
    /// Cells along one side of the square grid the maze is carved on.
    pub cells: usize,
    /// Side of one cell, in stage-local units.
    pub cell: f32,
    /// Thickness of a wall.
    pub wall_thickness: f32,
    /// The walls, merged into runs.
    pub walls: Vec<Wall>,
    /// Where a character is set down, stage-local.
    pub start: [f32; 2],
    /// Where the food is, stage-local.
    pub goal: [f32; 2],
    /// Cell centres from start to food. For scoring only.
    pub solution: Vec<[f32; 2]>,
    /// Length of `solution` in stage-local units, zero when unreachable.
    pub path_length: f32,
    /// The start and the food are the same cell, so there is nothing to do.
    pub degenerate: bool,
}

/// Wall bits per cell. North is `-z`, and the order matches the neighbour offsets.
const N: u8 = 1;
const E: u8 = 2;
const S: u8 = 4;
const W: u8 = 8;
const CLOSED: u8 = N | E | S | W;

/// Wall thickness as a fraction of a cell.
///
/// Kept thin on purpose. A corridor has to be wider than a character: she is
/// [`crate::editor::BODY_RADIUS`] in every direction, and a maze whose passages
/// are narrower than she is turns every round into a wall-bumping contest that
/// the collision resolver wins by pushing her back the way she came.
const WALL_FRACTION: f32 = 0.22;

/// The largest grid tried before giving up on fitting one inside the stage.
const MAX_CELLS: usize = 9;

/// A small deterministic generator, local to this module.
///
/// The core has its own, but a maze is rebuilt on the editor thread and has to
/// come out the same on every machine and every run, so it cannot depend on the C
/// brain's state. A 32-bit xorshift: no dependency, no allocation, and the same
/// output everywhere.
struct Rng(u32);

impl Rng {
    fn new(seed: u32) -> Self {
        // Zero is a fixed point of xorshift, so it is never allowed to be a seed.
        Self(seed | 1)
    }

    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    /// A value in `0..n`. `n` is never zero here, so the modulo cannot fail.
    fn below(&mut self, n: usize) -> usize {
        (self.next_u32() % n as u32) as usize
    }
}

/// The grid the maze is carved on, before it becomes walls.
struct Grid {
    cells: usize,
    cell: f32,
    half: f32,
    /// Radius of the disc the maze has to fit inside.
    radius: f32,
    /// Cell centres, in the order they were added.
    centres: Vec<[f32; 2]>,
    /// Grid slot to cell number, or `usize::MAX` when the slot is off the disc.
    index: Vec<usize>,
    /// Grid row of each cell.
    ///
    /// A cell number is a position in `centres`, not a grid slot, so the row and
    /// column have to be kept rather than recovered by dividing. Dividing gives
    /// a plausible-looking wrong answer, which is how the whole maze came out in
    /// pieces that could not be walked.
    row_of: Vec<usize>,
    /// Grid column of each cell.
    col_of: Vec<usize>,
    /// Remaining walls per cell.
    walls: Vec<u8>,
}

impl Grid {
    fn slot_is_inside(&self, row: usize, col: usize) -> bool {
        row < self.cells && col < self.cells && self.index[row * self.cells + col] != usize::MAX
    }

    fn at(&self, row: usize, col: usize) -> usize {
        self.index[row * self.cells + col]
    }

    /// The neighbour of a cell across one side, if that passage is open.
    fn open_neighbour(&self, cell: usize, side: u8) -> Option<usize> {
        if self.walls[cell] & side != 0 {
            return None;
        }
        let (dr, dc) = match side {
            N => (-1i64, 0i64),
            E => (0, 1),
            S => (1, 0),
            _ => (0, -1),
        };
        let nr = self.row_of[cell] as i64 + dr;
        let nc = self.col_of[cell] as i64 + dc;
        if nr < 0 || nc < 0 || nr >= self.cells as i64 || nc >= self.cells as i64 {
            return None;
        }
        if self.slot_is_inside(nr as usize, nc as usize) {
            Some(self.at(nr as usize, nc as usize))
        } else {
            None
        }
    }
}

impl Course {
    /// Build a maze inscribed in a disc of `radius`.
    ///
    /// The grid is square, the stage is round, and the corners of a square do not
    /// fit in a circle. Rather than clip the scenery afterwards, the cells whose
    /// centres fall outside the disc are simply not part of the maze and their
    /// sides are never carved, so the boundary follows the shape of the stage on
    /// its own.
    ///
    /// `cells` is a wish. It is reduced until the passages are wide enough for a
    /// character to walk down, because a maze that cannot be walked is scenery.
    pub fn build(seed: u32, cells: usize, radius: f32) -> Self {
        let mut cells = cells.clamp(3, MAX_CELLS);
        loop {
            let mut grid = Grid::new(cells, radius);
            let carved = grid.carve(seed);
            if !carved {
                // The disc cut the grid into nothing walkable. Back off and try
                // a smaller grid rather than returning a stage with no walls.
                if cells <= 3 {
                    return Self::empty(seed, cells, grid.cell, grid.cell * WALL_FRACTION);
                }
                cells -= 1;
                continue;
            }
            let (start, goal) = grid.endpoints();
            let solution = grid.solve(start, goal);
            let degenerate = start == goal;
            let path_length = length_of(&solution);
            let (start_point, goal_point) = (grid.centres[start], grid.centres[goal]);
            let walls = grid.walls();
            let cell = grid.cell;
            let wall_thickness = cell * WALL_FRACTION;
            return Self {
                seed,
                cells,
                cell,
                wall_thickness,
                walls,
                start: start_point,
                goal: goal_point,
                solution,
                path_length,
                degenerate,
            };
        }
    }

    fn empty(seed: u32, cells: usize, cell: f32, wall_thickness: f32) -> Self {
        Self {
            seed,
            cells,
            cell,
            wall_thickness,
            walls: Vec::new(),
            start: [0.0, 0.0],
            goal: [0.0, 0.0],
            solution: Vec::new(),
            path_length: 0.0,
            degenerate: true,
        }
    }

    /// The walls as circles for the collision pass, in stage-local coordinates.
    ///
    /// A slab is approximated by circles strung along its length, spaced so they
    /// just touch. That is what the existing, tested collision code already
    /// understands, and the circles are derived here from the very segments the
    /// client draws, so the two cannot drift apart.
    pub fn solids(&self) -> Vec<[f32; 3]> {
        let mut out = Vec::new();
        for wall in &self.walls {
            let r = wall.half_thick;
            if r <= 0.0 {
                continue;
            }
            let (sin, cos) = wall.angle.sin_cos();
            // One circle per diameter of wall, so there is no gap along the run.
            // One is always there, however short the wall.
            let span = wall.half_len * 2.0;
            let n = ((span / (r * 2.0)).ceil() as usize).max(1);
            for i in 0..n {
                let t = if n == 1 {
                    0.0
                } else {
                    -wall.half_len + span * (i as f32) / ((n - 1) as f32)
                };
                out.push([wall.x + cos * t, wall.z - sin * t, r]);
            }
        }
        out
    }
}

impl Grid {
    /// Lay out the cells that fall inside the disc, and shrink until a passage is
    /// wide enough to walk down.
    fn new(mut cells: usize, radius: f32) -> Self {
        // Leave a sliver of bare stage outside the maze so a character can be set
        // down on it and the outermost wall is visibly inside the rim.
        let mut cell = radius * 1.86 / cells as f32;
        // A character is BODY_RADIUS across twice over. Below that she grinds.
        while cell * (1.0 - WALL_FRACTION) < crate::editor::BODY_RADIUS * 2.2 && cells > 3 {
            cells -= 1;
            cell = radius * 1.86 / cells as f32;
        }
        let half = cell * cells as f32 * 0.5;
        let mut grid = Grid {
            cells,
            cell,
            half,
            radius,
            centres: Vec::new(),
            index: vec![usize::MAX; cells * cells],
            row_of: Vec::new(),
            col_of: Vec::new(),
            walls: Vec::new(),
        };
        let limit = (radius * 0.98).powi(2);
        for row in 0..cells {
            for col in 0..cells {
                let x = -half + (col as f32 + 0.5) * cell;
                let z = -half + (row as f32 + 0.5) * cell;
                if x * x + z * z > limit {
                    continue;
                }
                grid.index[row * cells + col] = grid.centres.len();
                grid.centres.push([x, z]);
                grid.row_of.push(row);
                grid.col_of.push(col);
            }
        }
        grid.walls = vec![CLOSED; grid.centres.len()];
        grid
    }

    /// Carve with a randomised depth-first walk.
    ///
    /// This produces a maze with exactly one route between any two cells, which
    /// is what makes a course a puzzle rather than a hedge: a character that
    /// arrives has found something, and there is no shortcut to stumble into.
    ///
    /// Returns false when the disc left too little to carve.
    fn carve(&mut self, seed: u32) -> bool {
        let cells = self.cells;
        if self.centres.len() < 4 {
            return false;
        }
        let mut rng = Rng::new(seed);
        let mut visited = vec![false; self.centres.len()];
        // (cell, row, col). The row and col are carried because the cell number
        // is a position in `centres`, not a grid slot, and the walk needs both.
        let mut stack: Vec<(usize, usize, usize)> = Vec::new();
        let first = rng.below(self.centres.len());
        visited[first] = true;
        stack.push((first, self.row_of[first], self.col_of[first]));
        // The side, and the row/col step, and the bit on this side and on the
        // other. The order is fixed so a seed gives one maze, not four.
        let steps: [(u8, i64, i64, u8); 4] =
            [(N, -1, 0, S), (E, 0, 1, W), (S, 1, 0, N), (W, 0, -1, E)];

        while let Some(&mut (cell, row, col)) = stack.last_mut() {
            let mut options: [(usize, usize, usize, u8, u8); 4] = [(0, 0, 0, 0, 0); 4];
            let mut found = 0;
            for &(here, dr, dc, there) in &steps {
                if self.walls[cell] & here == 0 {
                    continue;
                }
                let nr = row as i64 + dr;
                let nc = col as i64 + dc;
                if nr < 0 || nc < 0 || nr >= cells as i64 || nc >= cells as i64 {
                    continue;
                }
                if !self.slot_is_inside(nr as usize, nc as usize) {
                    continue;
                }
                let ni = self.at(nr as usize, nc as usize);
                if visited[ni] {
                    continue;
                }
                options[found] = (ni, nr as usize, nc as usize, here, there);
                found += 1;
            }
            if found == 0 {
                stack.pop();
                continue;
            }
            let pick = options[rng.below(found)];
            // Knock through both sides of the shared edge, or the passage is a
            // one-way wall and the maze is unsolvable.
            self.walls[cell] &= !pick.3;
            self.walls[pick.0] &= !pick.4;
            visited[pick.0] = true;
            stack.push((pick.0, pick.1, pick.2));
        }
        // Every inside cell has to have been reached, or the disc cut the grid
        // somewhere the walk could not cross and the maze is in pieces.
        visited.iter().all(|v| *v)
    }

    /// The two cells a round starts and ends on.
    ///
    /// These are the ends of the maze's longest path, not the west and east rims.
    /// A rim-to-rim course is only as interesting as the seed makes it: the walk
    /// came out barely longer than the straight line often enough to matter, and
    /// a course that can be solved by walking in a straight line is not a course.
    /// The longest path is found by walking out from every cell, which is a few
    /// hundred steps on a grid this size and is worth it to be sure the round is
    /// always a puzzle.
    ///
    /// Both ends come from one connected piece, so they are always reachable even
    /// if the disc cut the grid in two.
    fn endpoints(&self) -> (usize, usize) {
        let piece = self.piece_containing(0);
        let mut start = piece[0];
        let mut goal = piece[0];
        let mut longest = 0usize;
        for &from in &piece {
            let distances = self.distances_from(from);
            for &to in &piece {
                if distances[to] > longest {
                    longest = distances[to];
                    start = from;
                    goal = to;
                }
            }
        }
        (start, goal)
    }

    /// Hop count from `from` to every cell, `usize::MAX` where unreachable.
    fn distances_from(&self, from: usize) -> Vec<usize> {
        let mut out = vec![usize::MAX; self.centres.len()];
        if from >= self.centres.len() {
            return out;
        }
        out[from] = 0;
        let mut queue = std::collections::VecDeque::from([from]);
        while let Some(cell) = queue.pop_front() {
            let step = out[cell] + 1;
            for side in [N, E, S, W] {
                if let Some(next) = self.open_neighbour(cell, side)
                    && out[next] == usize::MAX
                {
                    out[next] = step;
                    queue.push_back(next);
                }
            }
        }
        out
    }

    /// Every cell reachable from `from` without crossing a wall.
    fn piece_containing(&self, from: usize) -> Vec<usize> {
        let mut seen = vec![false; self.centres.len()];
        let mut queue = std::collections::VecDeque::from([from]);
        seen[from] = true;
        let mut out = vec![from];
        while let Some(cell) = queue.pop_front() {
            for side in [N, E, S, W] {
                if let Some(next) = self.open_neighbour(cell, side)
                    && !seen[next]
                {
                    seen[next] = true;
                    out.push(next);
                    queue.push_back(next);
                }
            }
        }
        out
    }

    /// The unique route from `from` to `to`, as cell centres.
    ///
    /// The maze is a tree, so this finds the one route that exists. It walks
    /// parents rather than distances because a maze has a single path and a
    /// distance map would carry no more information than a straight line.
    fn solve(&self, from: usize, to: usize) -> Vec<[f32; 2]> {
        if from == to {
            return vec![self.centres[from]];
        }
        let mut parent = vec![usize::MAX; self.centres.len()];
        let mut seen = vec![false; self.centres.len()];
        let mut queue = std::collections::VecDeque::from([from]);
        seen[from] = true;
        while let Some(cell) = queue.pop_front() {
            if cell == to {
                break;
            }
            for side in [N, E, S, W] {
                if let Some(next) = self.open_neighbour(cell, side)
                    && !seen[next]
                {
                    seen[next] = true;
                    parent[next] = cell;
                    queue.push_back(next);
                }
            }
        }
        if !seen[to] {
            // Should not happen: both endpoints came from one connected piece.
            return Vec::new();
        }
        let mut path = vec![to];
        let mut at = to;
        while at != from {
            at = parent[at];
            if at == usize::MAX {
                return Vec::new();
            }
            path.push(at);
        }
        path.reverse();
        path.into_iter().map(|cell| self.centres[cell]).collect()
    }

    /// The walls, as merged runs.
    ///
    /// Every interior wall is shared by two cells, so it is emitted once by
    /// looking only at north and west sides, plus the far edges of the last row
    /// and column. Collinear runs are then merged, because a maze is mostly long
    /// straight walls and one slab per cell edge would both bloat the collision
    /// list and give the client a picket fence to draw where a wall belongs.
    fn walls(&self) -> Vec<Wall> {
        let mut out = Vec::new();
        let half_thick = self.cell * WALL_FRACTION * 0.5;

        // Horizontal walls, gathered row by row. Boundary `b` lies between row
        // `b - 1` and row `b`.
        for boundary in 0..=self.cells {
            let mut run: Option<(usize, usize)> = None;
            for col in 0..=self.cells {
                let has = self.horizontal_wall(boundary, col);
                match (run, has) {
                    (Some((from, _)), true) => run = Some((from, col)),
                    (Some((from, to)), false) => {
                        push_run(
                            self,
                            &mut out,
                            (from, to),
                            boundary,
                            half_thick,
                            Axis::Horizontal,
                        );
                        run = None;
                    }
                    (None, true) => run = Some((col, col)),
                    (None, false) => {}
                }
            }
        }
        // Vertical walls, gathered column by column.
        for boundary in 0..=self.cells {
            let mut run: Option<(usize, usize)> = None;
            for row in 0..=self.cells {
                let has = self.vertical_wall(boundary, row);
                match (run, has) {
                    (Some((from, _)), true) => run = Some((from, row)),
                    (Some((from, to)), false) => {
                        push_run(
                            self,
                            &mut out,
                            (from, to),
                            boundary,
                            half_thick,
                            Axis::Vertical,
                        );
                        run = None;
                    }
                    (None, true) => run = Some((row, row)),
                    (None, false) => {}
                }
            }
        }
        out
    }

    /// Is there a wall on the horizontal boundary `b` beside column `col`?
    ///
    /// It counts if either cell that would share it is part of the maze. That is
    /// what makes the outer boundary follow the disc: where both neighbours are
    /// off the stage there is nothing to hold back and no wall is drawn.
    fn horizontal_wall(&self, boundary: usize, col: usize) -> bool {
        if col >= self.cells {
            return false;
        }
        let above = boundary < self.cells && self.slot_is_inside(boundary, col);
        let below = boundary > 0 && self.slot_is_inside(boundary - 1, col);
        if !above && !below {
            return false;
        }
        let north = above && self.walls[self.at(boundary, col)] & N != 0;
        let south = below && self.walls[self.at(boundary - 1, col)] & S != 0;
        north || south
    }

    /// Is there a wall on the vertical boundary `b` beside row `row`?
    fn vertical_wall(&self, boundary: usize, row: usize) -> bool {
        if row >= self.cells {
            return false;
        }
        let left = boundary < self.cells && self.slot_is_inside(row, boundary);
        let right = boundary > 0 && self.slot_is_inside(row, boundary - 1);
        if !left && !right {
            return false;
        }
        let west = left && self.walls[self.at(row, boundary)] & W != 0;
        let east = right && self.walls[self.at(row, boundary - 1)] & E != 0;
        west || east
    }
}

/// Which way a run of walls lies.
#[derive(Clone, Copy, PartialEq)]
enum Axis {
    Horizontal,
    Vertical,
}

/// Turn one merged run of walls into a slab, clipped to the stage.
///
/// The run is turned into a slab here rather than in the caller because the
/// clipping needs the grid's geometry, and passing eight numbers about to every
/// call is how one of them ends up wrong in one direction only.
fn push_run(
    grid: &Grid,
    out: &mut Vec<Wall>,
    run: (usize, usize),
    boundary: usize,
    half_thick: f32,
    axis: Axis,
) {
    let (from, to) = run;
    // The run covers cells `from` through `to` inclusive, so it spans one cell
    // more than its length in cells. Cell edges sit at `-half + cell * n`, and
    // measuring from zero instead puts the whole maze half a grid off the stage
    // and its walls hanging in the air beyond the rim.
    let edge = |n: usize| -grid.half + grid.cell * (n as f32);
    let mut lo = edge(from);
    let mut hi = edge(to + 1);
    let across = edge(boundary);
    // Clip the run to the disc. The grid is a square and the stage is a circle,
    // so the outer wall of a run near a corner reaches past the rim. The limit is
    // on the slab's *corner*, not on its centre line: the end cap is a half
    // thickness further out than the wall, and clipping to the centre line leaves
    // corners hanging over the edge. What the clip takes away is unreachable
    // anyway, because the gap it leaves is the sliver between the maze and the
    // rim, which is narrower than a character.
    let out_reach = across.abs() + half_thick;
    let reach = (grid.radius * grid.radius - out_reach * out_reach)
        .max(0.0)
        .sqrt();
    if reach <= 0.0 {
        return;
    }
    lo = lo.max(-reach);
    hi = hi.min(reach);
    if hi <= lo {
        return;
    }
    let along = (hi - lo) * 0.5;
    let centre = (lo + hi) * 0.5;
    out.push(if axis == Axis::Vertical {
        Wall {
            x: across,
            z: centre,
            half_len: along,
            half_thick,
            angle: std::f32::consts::FRAC_PI_2,
        }
    } else {
        Wall {
            x: centre,
            z: across,
            half_len: along,
            half_thick,
            angle: 0.0,
        }
    });
}

fn length_of(path: &[[f32; 2]]) -> f32 {
    path.windows(2)
        .map(|pair| {
            let dx = pair[1][0] - pair[0][0];
            let dz = pair[1][1] - pair[0][1];
            (dx * dx + dz * dz).sqrt()
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RADIUS: f32 = 1.7;

    #[test]
    fn a_course_is_walkable_end_to_end() {
        // The whole point of the thing. A maze that does not connect its entrance
        // to its food is a hedge, and the character would mill inside it forever
        // with nothing to show for it.
        for seed in 0..60u32 {
            let course = Course::build(seed, 6, RADIUS);
            assert!(
                !course.degenerate,
                "seed {seed} put the food on top of the entrance"
            );
            assert!(
                course.solution.len() >= 2,
                "seed {seed} has no route: {:?}",
                course.solution
            );
            assert_eq!(
                course.solution.first().copied(),
                Some(course.start),
                "seed {seed} does not start where it says"
            );
            assert_eq!(
                course.solution.last().copied(),
                Some(course.goal),
                "seed {seed} does not end at the food"
            );
        }
    }

    #[test]
    fn every_passage_is_wider_than_a_character() {
        // A corridor narrower than she is turns the round into a wall-bumping
        // contest the collision resolver wins by pushing her back the way she
        // came. The generator shrinks the grid until this holds, so the test
        // checks the promise rather than the arithmetic.
        for cells in 3..=MAX_CELLS + 3 {
            for seed in [1u32, 17, 99] {
                let course = Course::build(seed, cells, RADIUS);
                let clear = course.cell - course.wall_thickness;
                assert!(
                    clear > crate::editor::BODY_RADIUS * 2.0,
                    "{cells} cells at seed {seed} gives a {clear:.3} passage, \
                     which is under a {BODY} wide character",
                    BODY = crate::editor::BODY_RADIUS * 2.0
                );
            }
        }
    }

    #[test]
    fn the_maze_stays_on_the_stage() {
        // The stage is a disc. A wall outside it is scenery the client draws
        // hanging in the air, and a food outside it can never be reached because
        // the rim keeps her in.
        for seed in 0..40u32 {
            let course = Course::build(seed, 7, RADIUS);
            for wall in &course.walls {
                // A wall is a slab, so its far corners have to fit on the disc,
                // not just its centre. Every wall this generator emits is axis
                // aligned, so each half-extent goes on its own axis.
                let (dx, dz) = if wall.angle.cos().abs() > 0.5 {
                    (wall.half_len, wall.half_thick)
                } else {
                    (wall.half_thick, wall.half_len)
                };
                let d = (wall.x.abs() + dx).hypot(wall.z.abs() + dz);
                assert!(
                    d <= RADIUS * 1.02,
                    "seed {seed} put a wall corner out to {d:.3} on a {RADIUS} stage"
                );
            }
            let food = course.goal[0].abs().max(course.goal[1].abs());
            assert!(
                food <= RADIUS,
                "seed {seed} put the food at {food:.3}, off the stage"
            );
        }
    }

    #[test]
    fn the_food_is_a_walk_not_a_step() {
        // The ends are the ends of the maze's longest path, so the route can run
        // diagonally and does not have to cross the stage the short way. What has
        // to hold is that there is a walk to do at all, rather than a food sitting
        // in the next cell.
        for seed in 0..40u32 {
            let course = Course::build(seed, 6, RADIUS);
            assert!(
                course.solution.len() >= 5,
                "seed {seed} asks for a walk of only {} cells",
                course.solution.len()
            );
            // The ends are measured in path length, not in distance. A maze is a
            // tree, so a long route can wander back near where it began and still
            // be the longest way through; how far apart the ends look on the
            // stage says nothing about the work.
            assert!(
                course.path_length > course.cell * 4.0,
                "seed {seed} is a walk of {:.2}, under four cells",
                course.path_length
            );
        }
    }

    #[test]
    fn the_route_is_longer_than_the_straight_line() {
        // A maze whose answer is the direct line is not a maze. This is a floor,
        // not an exact figure: the shape of the stage and the disc cut make the
        // real ratio vary, but nothing should be near one.
        for seed in 0..40u32 {
            let course = Course::build(seed, 6, RADIUS);
            let straight = {
                let dx = course.goal[0] - course.start[0];
                let dz = course.goal[1] - course.start[1];
                (dx * dx + dz * dz).sqrt()
            };
            assert!(
                course.path_length > straight * 1.2,
                "seed {seed} walks {:.2} to get {:.2} straight away",
                course.path_length,
                straight
            );
        }
    }

    #[test]
    fn the_same_seed_gives_the_same_maze() {
        // A round has to be repeatable, or a bug report is a description of a
        // maze nobody else will ever see.
        let a = Course::build(4242, 6, RADIUS);
        let b = Course::build(4242, 6, RADIUS);
        assert_eq!(a.walls, b.walls);
        assert_eq!(a.start, b.start);
        assert_eq!(a.goal, b.goal);
        assert_eq!(a.solution, b.solution);
    }

    #[test]
    fn different_seeds_give_different_mazes() {
        // The round is only worth repeating if the next one is a different
        // problem. This is what makes the course not the adventures it replaced.
        let reference = Course::build(1, 6, RADIUS);
        let mut different = 0;
        for seed in 2..30u32 {
            let course = Course::build(seed, 6, RADIUS);
            if course.walls != reference.walls {
                different += 1;
            }
        }
        assert!(
            different >= 25,
            "only {different} of 28 seeds differed from the first"
        );
    }

    #[test]
    fn the_walls_a_character_hits_are_the_walls_that_were_drawn() {
        // The collision circles are derived from the drawn slabs. If the
        // derivation dropped a wall or moved one, the character would walk
        // through something the client is still showing, which is the exact
        // failure the shared source was written to prevent.
        for seed in 0..20u32 {
            let course = Course::build(seed, 6, RADIUS);
            let solids = course.solids();
            assert!(
                !solids.is_empty(),
                "seed {seed} drew walls but collided against nothing"
            );
            for solid in &solids {
                assert!(
                    (solid[0].abs()).hypot(solid[1]) <= RADIUS * 1.02,
                    "seed {seed} put a collider off the stage at {solid:?}"
                );
                assert!(solid[2] > 0.0, "a collider of zero radius does nothing");
            }
            // Every drawn wall has to be covered by at least one circle, and the
            // circles have to stay close to it.
            for wall in &course.walls {
                let covered = solids.iter().any(|s| {
                    let dx = s[0] - wall.x;
                    let dz = s[1] - wall.z;
                    (dx * dx + dz * dz).sqrt() <= wall.half_len + wall.half_thick
                });
                assert!(covered, "seed {seed} drew a wall nothing collides with");
            }
        }
    }

    #[test]
    fn the_food_is_not_inside_a_wall() {
        // A food the character can never touch is a round that never ends. The
        // clearance is a body plus a little, which is the distance the
        // collision resolver will actually hold her off by.
        for seed in 0..40u32 {
            let course = Course::build(seed, 6, RADIUS);
            for solid in course.solids() {
                let dx = course.goal[0] - solid[0];
                let dz = course.goal[1] - solid[1];
                let clear = (dx * dx + dz * dz).sqrt() - solid[2];
                assert!(
                    clear > crate::editor::BODY_RADIUS * 0.5,
                    "seed {seed} put the food {clear:.3} inside a wall"
                );
            }
        }
    }

    #[test]
    fn the_entrance_is_not_inside_a_wall() {
        for seed in 0..40u32 {
            let course = Course::build(seed, 6, RADIUS);
            for solid in course.solids() {
                let dx = course.start[0] - solid[0];
                let dz = course.start[1] - solid[1];
                let clear = (dx * dx + dz * dz).sqrt() - solid[2];
                assert!(
                    clear > crate::editor::BODY_RADIUS * 0.5,
                    "seed {seed} set her down {clear:.3} inside a wall"
                );
            }
        }
    }
}
