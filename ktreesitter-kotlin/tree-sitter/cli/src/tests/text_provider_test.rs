use std::{iter, sync::Arc};

use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Node, Parser, Point, Query, QueryCursor, TextProvider, Tree};

use crate::tests::helpers::fixtures::get_language;

fn parse_text(text: impl AsRef<[u8]>) -> (Tree, Language) {
    let language = get_language("c");
    let mut parser = Parser::new();
    parser.set_language(&language).unwrap();
    (parser.parse(text, None).unwrap(), language)
}

fn parse_text_with<T, F>(callback: &mut F) -> (Tree, Language)
where
    T: AsRef<[u8]>,
    F: FnMut(usize, Point) -> T,
{
    let language = get_language("c");
    let mut parser = Parser::new();
    parser.set_language(&language).unwrap();
    let tree = parser.parse_with_options(callback, None, None).unwrap();
    // eprintln!("{}", tree.clone().root_node().to_sexp());
    assert_eq!("comment", tree.root_node().child(0).unwrap().kind());
    (tree, language)
}

fn tree_query<I: AsRef<[u8]>>(tree: &Tree, text: impl TextProvider<I>, language: &Language) {
    let query = Query::new(language, "((comment) @c (#eq? @c \"// comment\"))").unwrap();
    let mut cursor = QueryCursor::new();
    let mut captures = cursor.captures(&query, tree.root_node(), text);
    let (match_, idx) = captures.next().unwrap();
    let capture = match_.captures[*idx];
    assert_eq!(capture.index as usize, *idx);
    assert_eq!("comment", capture.node.kind());
}

fn check_parsing<I: AsRef<[u8]>>(
    parser_text: impl AsRef<[u8]>,
    text_provider: impl TextProvider<I>,
) {
    let (tree, language) = parse_text(parser_text);
    tree_query(&tree, text_provider, &language);
}

fn check_parsing_callback<T, F, I: AsRef<[u8]>>(
    parser_callback: &mut F,
    text_provider: impl TextProvider<I>,
) where
    T: AsRef<[u8]>,
    F: FnMut(usize, Point) -> T,
{
    let (tree, language) = parse_text_with(parser_callback);
    tree_query(&tree, text_provider, &language);
}

#[test]
fn test_text_provider_for_str_slice() {
    let text: &str = "// comment";

    check_parsing(text, text.as_bytes());
    check_parsing(text.as_bytes(), text.as_bytes());
}

#[test]
fn test_text_provider_for_string() {
    let text: String = "// comment".to_owned();

    check_parsing(text.clone(), text.as_bytes());
    check_parsing(text.as_bytes(), text.as_bytes());
    check_parsing(<_ as AsRef<[u8]>>::as_ref(&text), text.as_bytes());
}

#[test]
fn test_text_provider_for_box_of_str_slice() {
    let text = "// comment".to_owned().into_boxed_str();

    check_parsing(text.as_bytes(), text.as_bytes());
    check_parsing(<_ as AsRef<str>>::as_ref(&text), text.as_bytes());
    check_parsing(text.as_ref(), text.as_ref().as_bytes());
    check_parsing(text.as_ref(), text.as_bytes());
}

#[test]
fn test_text_provider_for_box_of_bytes_slice() {
    let text = "// comment".to_owned().into_boxed_str().into_boxed_bytes();

    check_parsing(text.as_ref(), text.as_ref());
    check_parsing(text.as_ref(), &*text);
    check_parsing(&*text, &*text);
}

#[test]
fn test_text_provider_for_vec_of_bytes() {
    let text = "// comment".to_owned().into_bytes();

    check_parsing(&*text, &*text);
}

#[test]
fn test_text_provider_for_arc_of_bytes_slice() {
    let text: Arc<[u8]> = Arc::from("// comment".to_owned().into_bytes());

    check_parsing(&*text, &*text);
    check_parsing(text.as_ref(), text.as_ref());
    check_parsing(text.clone(), text.as_ref());
}

#[test]
fn test_text_provider_for_vec_utf16_text() {
    let source_text = "你好".encode_utf16().collect::<Vec<_>>();

    let language = get_language("c");
    let mut parser = Parser::new();
    parser.set_language(&language).unwrap();
    let tree = parser.parse_utf16_le(&source_text, None).unwrap();

    let tree_text = tree.root_node().utf16_text(&source_text);
    assert_eq!(source_text, tree_text);
}

#[test]
fn test_text_provider_callback_with_str_slice() {
    let text: &str = "// comment";

    check_parsing(text, |_node: Node<'_>| iter::once(text));
    check_parsing_callback(
        &mut |offset, _point| {
            (offset < text.len())
                .then_some(text.as_bytes())
                .unwrap_or_default()
        },
        |_node: Node<'_>| iter::once(text),
    );
}

#[test]
fn test_text_provider_callback_with_owned_string_slice() {
    let text: &str = "// comment";

    check_parsing_callback(
        &mut |offset, _point| {
            (offset < text.len())
                .then_some(text.as_bytes())
                .unwrap_or_default()
        },
        |_node: Node<'_>| {
            let slice: String = text.to_owned();
            iter::once(slice)
        },
    );
}

#[test]
fn test_text_provider_callback_with_owned_bytes_vec_slice() {
    let text: &str = "// comment";

    check_parsing_callback(
        &mut |offset, _point| {
            (offset < text.len())
                .then_some(text.as_bytes())
                .unwrap_or_default()
        },
        |_node: Node<'_>| {
            let slice = text.to_owned().into_bytes();
            iter::once(slice)
        },
    );
}

#[test]
fn test_text_provider_callback_with_owned_arc_of_bytes_slice() {
    let text: &str = "// comment";

    check_parsing_callback(
        &mut |offset, _point| {
            (offset < text.len())
                .then_some(text.as_bytes())
                .unwrap_or_default()
        },
        |_node: Node<'_>| {
            let slice: Arc<[u8]> = text.to_owned().into_bytes().into();
            iter::once(slice)
        },
    );
}
