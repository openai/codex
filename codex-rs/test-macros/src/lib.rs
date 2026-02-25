use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::Attribute;
use syn::ItemFn;
use syn::parse::Nothing;
use syn::parse_macro_input;
use syn::parse_quote;

const LARGE_STACK_TEST_STACK_SIZE_BYTES: usize = 16 * 1024 * 1024;

/// Run a test body on a dedicated thread with a larger stack.
///
/// For async tests, this macro creates a Tokio multi-thread runtime with two
/// worker threads and blocks on the original async body inside the large-stack
/// thread.
#[proc_macro_attribute]
pub fn large_stack_test(attr: TokenStream, item: TokenStream) -> TokenStream {
    parse_macro_input!(attr as Nothing);

    let item = parse_macro_input!(item as ItemFn);
    expand_large_stack_test(item).into()
}

fn expand_large_stack_test(mut item: ItemFn) -> TokenStream2 {
    let attrs = filtered_attributes(&item.attrs);
    item.attrs = attrs;

    let is_async = item.sig.asyncness.take().is_some();
    let name = &item.sig.ident;
    let body = &item.block;

    let thread_body = if is_async {
        quote! {
            {
                let runtime = ::tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                    .unwrap_or_else(|error| {
                        panic!("failed to build tokio runtime for large-stack test: {error}")
                    });
                runtime.block_on(async move #body)
            }
        }
    } else {
        quote! { #body }
    };

    *item.block = parse_quote!({
        let handle = ::std::thread::Builder::new()
            .name(::std::string::String::from(::std::stringify!(#name)))
            .stack_size(#LARGE_STACK_TEST_STACK_SIZE_BYTES)
            .spawn(move || #thread_body)
            .unwrap_or_else(|error| {
                panic!("failed to spawn large-stack test thread: {error}")
            });

        match handle.join() {
            Ok(result) => result,
            Err(payload) => ::std::panic::resume_unwind(payload),
        }
    });

    quote! { #item }
}

fn filtered_attributes(attrs: &[Attribute]) -> Vec<Attribute> {
    let mut filtered = Vec::with_capacity(attrs.len() + 1);
    let mut has_test_attr = false;

    for attr in attrs {
        if is_tokio_test_attr(attr) {
            continue;
        }
        if is_test_attr(attr) || is_test_case_attr(attr) {
            has_test_attr = true;
        }
        filtered.push(attr.clone());
    }

    if !has_test_attr {
        filtered.push(parse_quote!(#[test]));
    }

    filtered
}

fn is_test_attr(attr: &Attribute) -> bool {
    attr.path().is_ident("test")
}

fn is_test_case_attr(attr: &Attribute) -> bool {
    attr.path().is_ident("test_case")
}

fn is_tokio_test_attr(attr: &Attribute) -> bool {
    let mut segments = attr.path().segments.iter();
    matches!(
        (segments.next(), segments.next(), segments.next()),
        (Some(first), Some(second), None) if first.ident == "tokio" && second.ident == "test"
    )
}

#[cfg(test)]
mod tests {
    use super::expand_large_stack_test;
    use syn::ItemFn;
    use syn::parse_quote;

    fn has_attr(item: &ItemFn, name: &str) -> bool {
        item.attrs.iter().any(|attr| attr.path().is_ident(name))
    }

    #[test]
    fn adds_test_attribute_when_missing() {
        let item: ItemFn = parse_quote! {
            fn sample() {}
        };

        let expanded_tokens = expand_large_stack_test(item);
        let expanded: ItemFn = match syn::parse2(expanded_tokens) {
            Ok(expanded) => expanded,
            Err(error) => panic!("failed to parse expanded function: {error}"),
        };

        assert!(has_attr(&expanded, "test"));
        let body = quote::quote!(#expanded).to_string();
        assert!(body.contains("stack_size"));
    }

    #[test]
    fn removes_tokio_test_and_keeps_test_case() {
        let item: ItemFn = parse_quote! {
            #[test_case(1)]
            #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
            async fn sample(value: usize) -> anyhow::Result<()> {
                let _ = value;
                Ok(())
            }
        };

        let expanded_tokens = expand_large_stack_test(item);
        let expanded: ItemFn = match syn::parse2(expanded_tokens) {
            Ok(expanded) => expanded,
            Err(error) => panic!("failed to parse expanded function: {error}"),
        };

        assert!(has_attr(&expanded, "test_case"));
        assert!(!has_attr(&expanded, "test"));
        let body = quote::quote!(#expanded).to_string();
        assert!(body.contains("tokio :: runtime :: Builder"));
        assert!(!body.contains("tokio :: test"));
    }
}
