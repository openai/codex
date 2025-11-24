use indoc::printdoc;

struct NoDisplay;

fn main() {
    printdoc!("{}", NoDisplay);
}
