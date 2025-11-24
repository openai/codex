use difference::{Changeset, Difference};
use yansi_term::Color::{Black, Green, Red};

pub fn get_diff(left: &str, right: &str) -> String {
    let mut output = String::new();

    let Changeset { diffs, .. } = Changeset::new(left, right, "\n");

    for i in 0..diffs.len() {
        match diffs[i] {
            Difference::Same(ref x) => {
                output += &format!(" {}\n", x);
            }
            Difference::Add(ref x) => {
                match diffs[i - 1] {
                    Difference::Rem(ref y) => {
                        output += &format!("{}", Green.paint("+"));
                        let Changeset { diffs, .. } = Changeset::new(y, x, " ");
                        for c in diffs {
                            match c {
                                Difference::Same(ref z) => {
                                    output += &format!("{} ", Green.paint(z.as_str()));
                                }
                                Difference::Add(ref z) => {
                                    output += &format!("{} ", Black.on(Green).paint(z.as_str()));
                                }
                                _ => (),
                            }
                        }
                        output += "\n";
                    }
                    _ => {
                        output += &format!("+{}\n", Green.paint(x.as_str()));
                    }
                };
            }
            Difference::Rem(ref x) => {
                output += &format!("-{}\n", Red.paint(x.as_str()));
            }
        }
    }
    output
}
