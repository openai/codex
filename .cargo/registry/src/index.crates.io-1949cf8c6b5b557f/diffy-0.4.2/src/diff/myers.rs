use crate::range::{DiffRange, Range};
use std::ops::{Index, IndexMut};

// A D-path is a path which starts at (0,0) that has exactly D non-diagonal edges. All D-paths
// consist of a (D - 1)-path followed by a non-diagonal edge and then a possibly empty sequence of
// diagonal edges called a snake.

/// `V` contains the endpoints of the furthest reaching `D-paths`. For each recorded endpoint
/// `(x,y)` in diagonal `k`, we only need to retain `x` because `y` can be computed from `x - k`.
/// In other words, `V` is an array of integers where `V[k]` contains the row index of the endpoint
/// of the furthest reaching path in diagonal `k`.
///
/// We can't use a traditional Vec to represent `V` since we use `k` as an index and it can take on
/// negative values. So instead `V` is represented as a light-weight wrapper around a Vec plus an
/// `offset` which is the maximum value `k` can take on in order to map negative `k`'s back to a
/// value >= 0.
#[derive(Debug, Clone)]
struct V {
    offset: isize,
    v: Vec<usize>, // Look into initializing this to -1 and storing isize
}

impl V {
    fn new(max_d: usize) -> Self {
        Self {
            offset: max_d as isize,
            v: vec![0; 2 * max_d],
        }
    }

    fn len(&self) -> usize {
        self.v.len()
    }
}

impl Index<isize> for V {
    type Output = usize;

    fn index(&self, index: isize) -> &Self::Output {
        &self.v[(index + self.offset) as usize]
    }
}

impl IndexMut<isize> for V {
    fn index_mut(&mut self, index: isize) -> &mut Self::Output {
        &mut self.v[(index + self.offset) as usize]
    }
}

/// A `Snake` is a sequence of diagonal edges in the edit graph. It is possible for a snake to have
/// a length of zero, meaning the start and end points are the same.
#[derive(Debug)]
struct Snake {
    x_start: usize,
    y_start: usize,
    x_end: usize,
    y_end: usize,
}

impl ::std::fmt::Display for Snake {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        write!(
            f,
            "({}, {}) -> ({}, {})",
            self.x_start, self.y_start, self.x_end, self.y_end
        )
    }
}

fn max_d(len1: usize, len2: usize) -> usize {
    // XXX look into reducing the need to have the additional '+ 1'
    (len1 + len2 + 1) / 2 + 1
}

// The divide part of a divide-and-conquer strategy. A D-path has D+1 snakes some of which may
// be empty. The divide step requires finding the ceil(D/2) + 1 or middle snake of an optimal
// D-path. The idea for doing so is to simultaneously run the basic algorithm in both the
// forward and reverse directions until furthest reaching forward and reverse paths starting at
// opposing corners 'overlap'.
fn find_middle_snake<T: PartialEq>(
    old: Range<'_, [T]>,
    new: Range<'_, [T]>,
    vf: &mut V,
    vb: &mut V,
) -> (isize, Snake) {
    let n = old.len();
    let m = new.len();

    // By Lemma 1 in the paper, the optimal edit script length is odd or even as `delta` is odd
    // or even.
    let delta = n as isize - m as isize;
    let odd = delta & 1 == 1;

    // The initial point at (0, -1)
    vf[1] = 0;
    // The initial point at (N, M+1)
    vb[1] = 0;

    // We only need to explore ceil(D/2) + 1
    let d_max = max_d(n, m);
    assert!(vf.len() >= d_max);
    assert!(vb.len() >= d_max);

    for d in 0..d_max as isize {
        // Forward path
        for k in (-d..=d).rev().step_by(2) {
            let mut x = if k == -d || (k != d && vf[k - 1] < vf[k + 1]) {
                vf[k + 1]
            } else {
                vf[k - 1] + 1
            };
            let mut y = (x as isize - k) as usize;

            // The coordinate of the start of a snake
            let (x0, y0) = (x, y);
            //  While these sequences are identical, keep moving through the graph with no cost
            if let (Some(s1), Some(s2)) = (old.get(x..), new.get(y..)) {
                let advance = s1.common_prefix_len(s2);
                x += advance;
                y += advance;
            }

            // This is the new best x value
            vf[k] = x;
            // Only check for connections from the forward search when N - M is odd
            // and when there is a reciprocal k line coming from the other direction.
            if odd && (k - delta).abs() <= (d - 1) {
                // TODO optimize this so we don't have to compare against n
                if vf[k] + vb[-(k - delta)] >= n {
                    // Return the snake
                    let snake = Snake {
                        x_start: x0,
                        y_start: y0,
                        x_end: x,
                        y_end: y,
                    };
                    // Edit distance to this snake is `2 * d - 1`
                    return (2 * d - 1, snake);
                }
            }
        }

        // Backward path
        for k in (-d..=d).rev().step_by(2) {
            let mut x = if k == -d || (k != d && vb[k - 1] < vb[k + 1]) {
                vb[k + 1]
            } else {
                vb[k - 1] + 1
            };
            let mut y = (x as isize - k) as usize;

            // The coordinate of the start of a snake
            let (x0, y0) = (x, y);
            if x < n && y < m {
                let advance = old.slice(..n - x).common_suffix_len(new.slice(..m - y));
                x += advance;
                y += advance;
            }

            // This is the new best x value
            vb[k] = x;

            if !odd && (k - delta).abs() <= d {
                // TODO optimize this so we don't have to compare against n
                if vb[k] + vf[-(k - delta)] >= n {
                    // Return the snake
                    let snake = Snake {
                        x_start: n - x,
                        y_start: m - y,
                        x_end: n - x0,
                        y_end: m - y0,
                    };
                    // Edit distance to this snake is `2 * d`
                    return (2 * d, snake);
                }
            }
        }

        // TODO: Maybe there's an opportunity to optimize and bail early?
    }

    unreachable!("unable to find a middle snake");
}

fn conquer<'a, 'b, T: PartialEq>(
    mut old: Range<'a, [T]>,
    mut new: Range<'b, [T]>,
    vf: &mut V,
    vb: &mut V,
    solution: &mut Vec<DiffRange<'a, 'b, [T]>>,
) {
    // Check for common prefix
    let common_prefix_len = old.common_prefix_len(new);
    if common_prefix_len > 0 {
        let common_prefix = DiffRange::Equal(
            old.slice(..common_prefix_len),
            new.slice(..common_prefix_len),
        );
        solution.push(common_prefix);
    }

    old = old.slice(common_prefix_len..old.len());
    new = new.slice(common_prefix_len..new.len());

    // Check for common suffix
    let common_suffix_len = old.common_suffix_len(new);
    let common_suffix = DiffRange::Equal(
        old.slice(old.len() - common_suffix_len..),
        new.slice(new.len() - common_suffix_len..),
    );
    old = old.slice(..old.len() - common_suffix_len);
    new = new.slice(..new.len() - common_suffix_len);

    if old.is_empty() && new.is_empty() {
        // Do nothing
    } else if old.is_empty() {
        // Inserts
        solution.push(DiffRange::Insert(new));
    } else if new.is_empty() {
        // Deletes
        solution.push(DiffRange::Delete(old));
    } else {
        // Divide & Conquer
        let (_shortest_edit_script_len, snake) = find_middle_snake(old, new, vf, vb);

        let (old_a, old_b) = old.split_at(snake.x_start);
        let (new_a, new_b) = new.split_at(snake.y_start);

        conquer(old_a, new_a, vf, vb, solution);
        conquer(old_b, new_b, vf, vb, solution);
    }

    if common_suffix_len > 0 {
        solution.push(common_suffix);
    }
}

pub fn diff<'a, 'b, T: PartialEq>(old: &'a [T], new: &'b [T]) -> Vec<DiffRange<'a, 'b, [T]>> {
    let old_recs = Range::new(old, ..);
    let new_recs = Range::new(new, ..);

    let mut solution = Vec::new();

    // The arrays that hold the 'best possible x values' in search from:
    // `vf`: top left to bottom right
    // `vb`: bottom right to top left
    let max_d = max_d(old.len(), new.len());
    let mut vf = V::new(max_d);
    let mut vb = V::new(max_d);

    conquer(old_recs, new_recs, &mut vf, &mut vb, &mut solution);

    solution
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_middle_snake() {
        let a = Range::new(&b"ABCABBA"[..], ..);
        let b = Range::new(&b"CBABAC"[..], ..);
        let max_d = max_d(a.len(), b.len());
        let mut vf = V::new(max_d);
        let mut vb = V::new(max_d);
        find_middle_snake(a, b, &mut vf, &mut vb);
    }
}
