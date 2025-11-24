use backtrace::Backtrace;

fn main() {
    println!("{:?}", Backtrace::new());
}
