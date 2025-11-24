use askama_parser::{Ast, Syntax};
use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};

criterion_main!(benches);
criterion_group!(benches, librustdoc);

fn librustdoc(c: &mut Criterion) {
    let mut group = c.benchmark_group("librustdoc");

    let mut add_benchmark = |name: &str, src: &str| {
        group.throughput(Throughput::Bytes(src.len() as u64));
        group.bench_function(name, |b| {
            let syntax = &Syntax::default();
            b.iter(|| Ast::from_str(black_box(src), None, black_box(syntax)).unwrap());
        });
    };

    let all: String = LIBRUSTDOC.iter().map(|&(_, src)| src).collect();
    add_benchmark("all", &all);

    for (name, src) in LIBRUSTDOC {
        add_benchmark(name, src);
    }

    group.finish();
}

const LIBRUSTDOC: &[(&str, &str)] = &[
    ("item_info", include_str!("./librustdoc/item_info.html")),
    ("item_union", include_str!("./librustdoc/item_union.html")),
    ("page", include_str!("./librustdoc/page.html")),
    ("print_item", include_str!("./librustdoc/print_item.html")),
    (
        "short_item_info",
        include_str!("./librustdoc/short_item_info.html"),
    ),
    ("sidebar", include_str!("./librustdoc/sidebar.html")),
    ("source", include_str!("./librustdoc/source.html")),
    (
        "type_layout_size",
        include_str!("./librustdoc/type_layout_size.html"),
    ),
    ("type_layout", include_str!("./librustdoc/type_layout.html")),
];
