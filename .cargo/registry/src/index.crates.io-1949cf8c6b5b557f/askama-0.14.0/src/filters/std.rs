use core::convert::Infallible;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::hash::Hash;
use std::rc::Rc;

/// Returns an iterator with all duplicates removed.
///
/// The sorting order is kept, no extra allocation is performed. However, to make it possible,
/// the data is wrapped inside `Rc`.
///
/// ```
/// # use askama::Template;
/// #[derive(Template)]
/// #[template(ext = "html", source = "{% for elem in example|unique %}{{ elem }},{% endfor %}")]
/// struct Example<'a> {
///     example: Vec<&'a str>,
/// }
///
/// assert_eq!(
///     Example { example: vec!["a", "b", "a", "c"] }.to_string(),
///     "a,b,c,"
/// );
/// ```
pub fn unique<T: Hash + Eq>(
    it: impl IntoIterator<Item = T>,
) -> Result<impl Iterator<Item = Rc<T>>, Infallible> {
    let mut set = HashMap::new();

    Ok(it.into_iter().filter_map(move |elem| {
        // To prevent cloning the data, we need to use `Rc`, like that we can clone `elem` as
        // key of the `HashSet` and return it.
        if let Entry::Vacant(entry) = set.entry(Rc::new(elem)) {
            Some(Rc::clone(entry.insert_entry(()).key()))
        } else {
            None
        }
    }))
}

#[cfg(test)]
mod test {
    use alloc::vec; // It's the macro, not the module.
    use alloc::vec::Vec;

    use super::*;

    #[test]
    fn test_unique() {
        assert_eq!(
            unique(["a", "b", "a", "c"]).unwrap().collect::<Vec<_>>(),
            vec![Rc::new("a"), Rc::new("b"), Rc::new("c")]
        );
        assert_eq!(
            unique([1, 1, 1, 2, 1]).unwrap().collect::<Vec<_>>(),
            vec![Rc::new(1), Rc::new(2)]
        );
        assert_eq!(
            unique("hello".chars()).unwrap().collect::<Vec<_>>(),
            vec![Rc::new('h'), Rc::new('e'), Rc::new('l'), Rc::new('o')]
        );
    }
}
