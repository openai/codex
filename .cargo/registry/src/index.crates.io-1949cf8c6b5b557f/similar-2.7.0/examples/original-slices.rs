use similar::utils::diff_chars;
use similar::Algorithm;

fn main() {
    let old = "1234567890abcdef".to_string();
    let new = "0123456789Oabzdef".to_string();

    for (change_tag, value) in diff_chars(Algorithm::Myers, &old, &new) {
        println!("{}{:?}", change_tag, value);
    }
}
