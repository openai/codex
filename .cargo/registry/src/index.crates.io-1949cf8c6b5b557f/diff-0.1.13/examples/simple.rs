extern crate diff;

fn main() {
    let left = "foo\nbar\nbaz\nquux";
    let right = "foo\nbaz\nbar\nquux";

    for diff in diff::lines(left, right) {
        match diff {
            diff::Result::Left(l) => println!("-{}", l),
            diff::Result::Both(l, _) => println!(" {}", l),
            diff::Result::Right(r) => println!("+{}", r),
        }
    }
}
