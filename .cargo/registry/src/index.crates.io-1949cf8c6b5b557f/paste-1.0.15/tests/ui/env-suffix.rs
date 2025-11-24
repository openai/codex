use paste::paste;

paste! {
    fn [<env!("VAR"suffix)>]() {}
}

fn main() {}
