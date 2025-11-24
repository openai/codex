use paste::paste;

paste! {
    fn [<0 f>]() {}
}

paste! {
    fn [<f '"'>]() {}
}

paste! {
    fn [<f "'">]() {}
}

fn main() {}
