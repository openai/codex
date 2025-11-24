use paste::paste;

paste! {
    fn [<env!("VAR" "VAR")>]() {}
}

fn main() {}
