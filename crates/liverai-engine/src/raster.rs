//! Small raster toolkit: grids, masks, connected components, morphology and an exact
//! Euclidean distance transform. These replace the NumPy/SciPy calls the proof-of-concept
//! scripts used, with the same conventions (4-connectivity, raster-order labels).

/// A dense row-major grid.
#[derive(Clone, Debug, PartialEq)]
pub struct Grid<T> {
    pub w: usize,
    pub h: usize,
    pub data: Vec<T>,
}

impl<T: Clone> Grid<T> {
    pub fn new(w: usize, h: usize, fill: T) -> Self {
        Self {
            w,
            h,
            data: vec![fill; w * h],
        }
    }

    #[inline]
    pub fn idx(&self, x: usize, y: usize) -> usize {
        y * self.w + x
    }

    #[inline]
    pub fn at(&self, x: usize, y: usize) -> &T {
        &self.data[y * self.w + x]
    }

    pub fn map<U: Clone>(&self, f: impl Fn(&T) -> U) -> Grid<U> {
        Grid {
            w: self.w,
            h: self.h,
            data: self.data.iter().map(f).collect(),
        }
    }
}

pub type Mask = Grid<bool>;

impl Mask {
    pub fn count(&self) -> usize {
        self.data.iter().filter(|v| **v).count()
    }

    pub fn not(&self) -> Mask {
        self.map(|v| !*v)
    }

    pub fn and(&self, other: &Mask) -> Mask {
        debug_assert_eq!(self.data.len(), other.data.len());
        Mask {
            w: self.w,
            h: self.h,
            data: self
                .data
                .iter()
                .zip(&other.data)
                .map(|(a, b)| *a && *b)
                .collect(),
        }
    }

    /// Number of set pixels inside the half-open box `[x0, x1) x [y0, y1)`, clamped to the grid.
    pub fn count_in_box(&self, x0: i64, y0: i64, x1: i64, y1: i64) -> usize {
        let (x0, x1) = (
            x0.clamp(0, self.w as i64) as usize,
            x1.clamp(0, self.w as i64) as usize,
        );
        let (y0, y1) = (
            y0.clamp(0, self.h as i64) as usize,
            y1.clamp(0, self.h as i64) as usize,
        );
        let mut n = 0;
        for y in y0..y1 {
            let row = &self.data[y * self.w..(y + 1) * self.w];
            n += row[x0..x1.max(x0)].iter().filter(|v| **v).count();
        }
        n
    }
}

/// Half-open pixel box `[x0, y0, x1, y1)`.
pub type BBox = [usize; 4];

/// Label 4-connected components. Labels start at 1 and are numbered in raster order of each
/// component's first pixel; 0 is background.
pub fn label4(mask: &Mask) -> (Grid<u32>, u32) {
    let (w, h) = (mask.w, mask.h);
    let mut labels = Grid::new(w, h, 0u32);
    let mut next = 0u32;
    let mut stack: Vec<usize> = Vec::new();
    for start in 0..w * h {
        if !mask.data[start] || labels.data[start] != 0 {
            continue;
        }
        next += 1;
        labels.data[start] = next;
        stack.push(start);
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            let mut visit = |j: usize| {
                if mask.data[j] && labels.data[j] == 0 {
                    labels.data[j] = next;
                    stack.push(j);
                }
            };
            if x > 0 {
                visit(i - 1);
            }
            if x + 1 < w {
                visit(i + 1);
            }
            if y > 0 {
                visit(i - w);
            }
            if y + 1 < h {
                visit(i + w);
            }
        }
    }
    (labels, next)
}

/// Pixel count per label (index 0 = label 1).
pub fn label_areas(labels: &Grid<u32>, n: u32) -> Vec<usize> {
    let mut areas = vec![0usize; n as usize];
    for &l in &labels.data {
        if l > 0 {
            areas[(l - 1) as usize] += 1;
        }
    }
    areas
}

/// Bounding box per label (index 0 = label 1). `None` if the label has no pixels.
pub fn label_bboxes(labels: &Grid<u32>, n: u32) -> Vec<Option<BBox>> {
    let mut boxes: Vec<Option<BBox>> = vec![None; n as usize];
    for y in 0..labels.h {
        for x in 0..labels.w {
            let l = labels.data[y * labels.w + x];
            if l == 0 {
                continue;
            }
            let b = boxes[(l - 1) as usize].get_or_insert([x, y, x + 1, y + 1]);
            b[0] = b[0].min(x);
            b[1] = b[1].min(y);
            b[2] = b[2].max(x + 1);
            b[3] = b[3].max(y + 1);
        }
    }
    boxes
}

/// Bounding boxes of the connected components of a mask, in raster order.
pub fn component_boxes(mask: &Mask) -> Vec<BBox> {
    let (labels, n) = label4(mask);
    label_bboxes(&labels, n).into_iter().flatten().collect()
}

fn box_pass(mask: &Mask, r: usize, horizontal: bool, dilate: bool) -> Mask {
    let (w, h) = (mask.w, mask.h);
    let mut out = Grid::new(w, h, false);
    let (outer, inner) = if horizontal { (h, w) } else { (w, h) };
    let window = 2 * r + 1;
    let mut line = vec![false; inner];
    for o in 0..outer {
        for (i, v) in line.iter_mut().enumerate() {
            *v = if horizontal {
                mask.data[o * w + i]
            } else {
                mask.data[i * w + o]
            };
        }
        // Prefix sums of set pixels along the line.
        let mut prefix = vec![0usize; inner + 1];
        for i in 0..inner {
            prefix[i + 1] = prefix[i] + line[i] as usize;
        }
        for i in 0..inner {
            let lo = i.saturating_sub(r);
            let hi = (i + r + 1).min(inner);
            let set = prefix[hi] - prefix[lo];
            let v = if dilate {
                set > 0
            } else {
                // Outside the grid counts as unset, so a clipped window can never be full.
                hi - lo == window && set == window
            };
            if horizontal {
                out.data[o * w + i] = v;
            } else {
                out.data[i * w + o] = v;
            }
        }
    }
    out
}

/// Dilate with a `(2r+1) x (2r+1)` square.
pub fn dilate_square(mask: &Mask, r: usize) -> Mask {
    box_pass(&box_pass(mask, r, true, true), r, false, true)
}

/// Erode with a `(2r+1) x (2r+1)` square. Pixels outside the grid count as unset.
pub fn erode_square(mask: &Mask, r: usize) -> Mask {
    box_pass(&box_pass(mask, r, true, false), r, false, false)
}

/// Morphological closing (dilate, then erode) with a `(2r+1) x (2r+1)` square.
pub fn close_square(mask: &Mask, r: usize) -> Mask {
    erode_square(&dilate_square(mask, r), r)
}

/// 3x3 maximum filter (edges clamp).
pub fn max3x3(grid: &Grid<u32>) -> Grid<u32> {
    let (w, h) = (grid.w, grid.h);
    let mut tmp = Grid::new(w, h, 0u32);
    for y in 0..h {
        for x in 0..w {
            let a = grid.data[y * w + x.saturating_sub(1)];
            let b = grid.data[y * w + x];
            let c = grid.data[y * w + (x + 1).min(w - 1)];
            tmp.data[y * w + x] = a.max(b).max(c);
        }
    }
    let mut out = Grid::new(w, h, 0u32);
    for y in 0..h {
        for x in 0..w {
            let a = tmp.data[y.saturating_sub(1) * w + x];
            let b = tmp.data[y * w + x];
            let c = tmp.data[(y + 1).min(h - 1) * w + x];
            out.data[y * w + x] = a.max(b).max(c);
        }
    }
    out
}

/// 3x3 minimum filter (edges clamp).
pub fn min3x3(grid: &Grid<u32>) -> Grid<u32> {
    let inv = grid.map(|v| u32::MAX - *v);
    max3x3(&inv).map(|v| u32::MAX - *v)
}

const INF: f64 = 1e20;

/// 1-D squared distance transform (Felzenszwalb & Huttenlocher) with arg-min tracking.
fn dt1d(f: &[f64], d: &mut [f64], arg: &mut [usize], v: &mut [usize], z: &mut [f64]) {
    let n = f.len();
    let mut k = 0usize;
    v[0] = 0;
    z[0] = -INF;
    z[1] = INF;
    for q in 1..n {
        loop {
            let p = v[k];
            let s =
                ((f[q] + (q * q) as f64) - (f[p] + (p * p) as f64)) / (2.0 * (q as f64 - p as f64));
            if s <= z[k] && k > 0 {
                k -= 1;
            } else {
                k += 1;
                v[k] = q;
                z[k] = s;
                z[k + 1] = INF;
                break;
            }
        }
    }
    k = 0;
    for q in 0..n {
        while z[k + 1] < q as f64 {
            k += 1;
        }
        let p = v[k];
        let dq = q as f64 - p as f64;
        d[q] = dq * dq + f[p];
        arg[q] = p;
    }
}

/// Exact Euclidean distance transform.
///
/// For every pixel, returns the distance to the nearest *unset* pixel of `mask` (0 for unset
/// pixels) and the linear index of that nearest unset pixel. If the mask has no unset pixel,
/// every distance is infinite and the index is the pixel itself.
pub fn edt(mask: &Mask) -> (Grid<f64>, Grid<usize>) {
    let (w, h) = (mask.w, mask.h);
    let n = w.max(h);
    let mut col_d = vec![0.0f64; w * h];
    let mut col_arg = vec![0usize; w * h];
    let (mut f, mut d, mut arg) = (vec![0.0; n], vec![0.0; n], vec![0usize; n]);
    let (mut v, mut z) = (vec![0usize; n], vec![0.0f64; n + 1]);

    for x in 0..w {
        for y in 0..h {
            f[y] = if mask.data[y * w + x] { INF } else { 0.0 };
        }
        dt1d(
            &f[..h],
            &mut d[..h],
            &mut arg[..h],
            &mut v[..h],
            &mut z[..h + 1],
        );
        for y in 0..h {
            col_d[y * w + x] = d[y];
            col_arg[y * w + x] = arg[y];
        }
    }

    let mut dist = Grid::new(w, h, 0.0f64);
    let mut nearest = Grid::new(w, h, 0usize);
    for y in 0..h {
        f[..w].copy_from_slice(&col_d[y * w..(y + 1) * w]);
        dt1d(
            &f[..w],
            &mut d[..w],
            &mut arg[..w],
            &mut v[..w],
            &mut z[..w + 1],
        );
        for x in 0..w {
            let i = y * w + x;
            if d[x] >= INF {
                dist.data[i] = f64::INFINITY;
                nearest.data[i] = i;
            } else {
                dist.data[i] = d[x].sqrt();
                let sx = arg[x];
                let sy = col_arg[y * w + sx];
                nearest.data[i] = sy * w + sx;
            }
        }
    }
    (dist, nearest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mask_from(rows: &[&str]) -> Mask {
        let h = rows.len();
        let w = rows[0].len();
        let data = rows
            .iter()
            .flat_map(|r| r.chars().map(|c| c == '#'))
            .collect();
        Mask { w, h, data }
    }

    #[test]
    fn labels_are_raster_ordered_and_4_connected() {
        let m = mask_from(&["#..#", "#.#.", "...#"]);
        let (labels, n) = label4(&m);
        // Diagonal neighbours are separate components.
        assert_eq!(n, 4);
        assert_eq!(labels.data, vec![1, 0, 0, 2, 1, 0, 3, 0, 0, 0, 0, 4]);
        assert_eq!(label_areas(&labels, n), vec![2, 1, 1, 1]);
        assert_eq!(label_bboxes(&labels, n)[0], Some([0, 0, 1, 2]));
    }

    #[test]
    fn edt_matches_brute_force() {
        let m = mask_from(&[
            "########", "##.#####", "########", "########", "######.#", "########",
        ]);
        let (dist, nearest) = edt(&m);
        let zeros = [(2usize, 1usize), (6, 4)];
        for y in 0..m.h {
            for x in 0..m.w {
                let brute = zeros
                    .iter()
                    .map(|(zx, zy)| {
                        (((x as f64 - *zx as f64).powi(2)) + ((y as f64 - *zy as f64).powi(2)))
                            .sqrt()
                    })
                    .fold(f64::INFINITY, f64::min);
                let i = y * m.w + x;
                assert!((dist.data[i] - brute).abs() < 1e-9, "({x},{y})");
                let (nx, ny) = (nearest.data[i] % m.w, nearest.data[i] / m.w);
                let via =
                    (((x as f64 - nx as f64).powi(2)) + ((y as f64 - ny as f64).powi(2))).sqrt();
                assert!((via - brute).abs() < 1e-9);
                assert!(!m.data[nearest.data[i]]);
            }
        }
    }

    #[test]
    fn edt_without_background_is_infinite() {
        let m = Mask::new(3, 3, true);
        let (dist, _) = edt(&m);
        assert!(dist.data.iter().all(|d| d.is_infinite()));
    }

    #[test]
    fn closing_fills_small_gaps_and_erosion_treats_outside_as_unset() {
        let mut m = Mask::new(20, 9, false);
        for x in 3..17 {
            if x != 9 {
                m.data[4 * 20 + x] = true;
            }
        }
        let closed = close_square(&m, 1);
        assert!(closed.data[4 * 20 + 9], "one-pixel gap is closed");
        assert_eq!(closed.count(), 14);

        let full = Mask::new(5, 5, true);
        let eroded = erode_square(&full, 1);
        assert_eq!(eroded.count(), 9, "border ring is eroded");
    }

    #[test]
    fn max_filter_spreads_labels() {
        let mut g = Grid::new(3, 3, 0u32);
        g.data[4] = 7;
        assert!(max3x3(&g).data.iter().all(|v| *v == 7));
        assert_eq!(min3x3(&g).data.iter().filter(|v| **v == 7).count(), 0);
    }

    #[test]
    fn count_in_box_clamps() {
        let m = Mask::new(4, 4, true);
        assert_eq!(m.count_in_box(-5, -5, 2, 2), 4);
        assert_eq!(m.count_in_box(3, 3, 99, 99), 1);
        assert_eq!(m.count_in_box(2, 2, 1, 1), 0);
    }
}
