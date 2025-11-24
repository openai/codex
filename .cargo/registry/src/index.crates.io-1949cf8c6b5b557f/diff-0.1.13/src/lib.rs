/// A fragment of a computed diff.
#[derive(Clone, Debug, PartialEq)]
pub enum Result<T> {
    /// An element that only exists in the left input.
    Left(T),
    /// Elements that exist in both inputs.
    Both(T, T),
    /// An element that only exists in the right input.
    Right(T),
}

/// Computes the diff between two slices.
pub fn slice<'a, T: PartialEq>(left: &'a [T], right: &'a [T]) -> Vec<Result<&'a T>> {
    do_diff(left, right, |t| t)
}

/// Computes the diff between the lines of two strings.
pub fn lines<'a>(left: &'a str, right: &'a str) -> Vec<Result<&'a str>> {
    let mut diff = do_diff(
        &left.lines().collect::<Vec<_>>(),
        &right.lines().collect::<Vec<_>>(),
        |str| *str,
    );
    // str::lines() does not yield an empty str at the end if the str ends with
    // '\n'. We handle this special case by inserting one last diff item,
    // depending on whether the left string ends with '\n', or the right one,
    // or both.
    match (
        left.as_bytes().last().cloned(),
        right.as_bytes().last().cloned(),
    ) {
        (Some(b'\n'), Some(b'\n')) => {
            diff.push(Result::Both(&left[left.len()..], &right[right.len()..]))
        }
        (Some(b'\n'), _) => diff.push(Result::Left(&left[left.len()..])),
        (_, Some(b'\n')) => diff.push(Result::Right(&right[right.len()..])),
        _ => {}
    }
    diff
}

/// Computes the diff between the chars of two strings.
pub fn chars<'a>(left: &'a str, right: &'a str) -> Vec<Result<char>> {
    do_diff(
        &left.chars().collect::<Vec<_>>(),
        &right.chars().collect::<Vec<_>>(),
        |char| *char,
    )
}

fn do_diff<'a, T, F, U>(left: &'a [T], right: &'a [T], mapper: F) -> Vec<Result<U>>
where
    T: PartialEq,
    F: Fn(&'a T) -> U,
{
    let leading_equals = left
        .iter()
        .zip(right.iter())
        .take_while(|(l, r)| l == r)
        .count();
    let trailing_equals = left[leading_equals..]
        .iter()
        .rev()
        .zip(right[leading_equals..].iter().rev())
        .take_while(|(l, r)| l == r)
        .count();

    let table: Vec2<u32> = {
        let left_diff_size = left.len() - leading_equals - trailing_equals;
        let right_diff_size = right.len() - leading_equals - trailing_equals;

        let mut table = Vec2::new(0, [left_diff_size + 1, right_diff_size + 1]);

        let left_skip = &left[leading_equals..left.len() - trailing_equals];
        let right_skip = &right[leading_equals..right.len() - trailing_equals];

        for (i, l) in left_skip.iter().enumerate() {
            for (j, r) in right_skip.iter().enumerate() {
                table.set(
                    [i + 1, j + 1],
                    if l == r {
                        table.get([i, j]) + 1
                    } else {
                        *table.get([i, j + 1]).max(table.get([i + 1, j]))
                    },
                );
            }
        }

        table
    };

    let mut diff = Vec::with_capacity(left.len().max(right.len()));

    diff.extend(
        left[..leading_equals]
            .iter()
            .zip(&right[..leading_equals])
            .map(|(l, r)| Result::Both(mapper(l), mapper(r))),
    );

    {
        let start = diff.len();
        let mut i = table.len[0] - 1;
        let mut j = table.len[1] - 1;
        let left = &left[leading_equals..];
        let right = &right[leading_equals..];

        loop {
            if j > 0 && (i == 0 || table.get([i, j]) == table.get([i, j - 1])) {
                j -= 1;
                diff.push(Result::Right(mapper(&right[j])));
            } else if i > 0 && (j == 0 || table.get([i, j]) == table.get([i - 1, j])) {
                i -= 1;
                diff.push(Result::Left(mapper(&left[i])));
            } else if i > 0 && j > 0 {
                i -= 1;
                j -= 1;
                diff.push(Result::Both(mapper(&left[i]), mapper(&right[j])));
            } else {
                break;
            }
        }
        diff[start..].reverse();
    }

    diff.extend(
        left[left.len() - trailing_equals..]
            .iter()
            .zip(&right[right.len() - trailing_equals..])
            .map(|(l, r)| Result::Both(mapper(l), mapper(r))),
    );

    diff
}

struct Vec2<T> {
    len: [usize; 2],
    data: Vec<T>,
}

impl<T> Vec2<T> {
    #[inline]
    fn new(value: T, len: [usize; 2]) -> Self
    where
        T: Clone,
    {
        Vec2 {
            len,
            data: vec![value; len[0] * len[1]],
        }
    }

    #[inline]
    fn get(&self, index: [usize; 2]) -> &T {
        debug_assert!(index[0] < self.len[0]);
        debug_assert!(index[1] < self.len[1]);
        &self.data[index[0] * self.len[1] + index[1]]
    }

    #[inline]
    fn set(&mut self, index: [usize; 2], value: T) {
        debug_assert!(index[0] < self.len[0]);
        debug_assert!(index[1] < self.len[1]);
        self.data[index[0] * self.len[1] + index[1]] = value;
    }
}
