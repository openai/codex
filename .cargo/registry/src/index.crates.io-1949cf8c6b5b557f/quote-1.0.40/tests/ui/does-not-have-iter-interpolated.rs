use quote::quote;

fn main() {
    let nonrep = "";

    // Without some protection against repetitions with no iterator somewhere
    // inside, this would loop infinitely.
    quote!(#(#nonrep)*);
}
