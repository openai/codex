use similar::{capture_diff_slices, Algorithm};

fn main() {
    let old = vec![1, 2, 3];
    let new = vec![1, 2, 4];
    let ops = capture_diff_slices(Algorithm::Myers, &old, &new);

    for op in ops {
        for change in op.iter_changes(&old, &new) {
            println!("{:?}", change);
        }
    }
}
