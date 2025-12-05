use std::io::Read as _;

fn main() {
    let mut stdin = String::new();
    std::io::stdin().read_to_string(&mut stdin).unwrap();

    let term = anstyle_svg::Term::new();
    let stdout = term.render_svg(&stdin);

    print!("{stdout}");
}
