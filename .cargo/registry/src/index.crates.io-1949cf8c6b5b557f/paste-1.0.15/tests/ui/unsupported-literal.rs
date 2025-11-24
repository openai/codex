use paste::paste;

paste! {
    fn [<x 1e+100 z>]() {}
}

paste! {
    // `xyz` is not correct. `xbyz` is certainly not correct. Maybe `x121z`
    // would be justifiable but for now don't accept this.
    fn [<x b'y' z>]() {}
}

paste! {
    fn [<x b"y" z>]() {}
}

paste! {
    fn [<x br"y" z>]() {}
}

fn main() {}
