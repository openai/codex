use askama::Template;
use criterion::{Criterion, black_box, criterion_group, criterion_main};

criterion_main!(benches);
criterion_group!(benches, functions);

fn functions(c: &mut Criterion) {
    c.bench_function("escape JSON", escape_json);
    c.bench_function("escape JSON (pretty)", escape_json_pretty);
    c.bench_function("escape JSON for HTML", escape_json_for_html);
    c.bench_function("escape JSON for HTML (pretty)", escape_json_for_html_pretty);
}

fn escape_json(b: &mut criterion::Bencher<'_>) {
    #[derive(Template)]
    #[template(ext = "html", source = "{{self.0|json|safe}}")]
    struct Tmpl(&'static str);

    b.iter(|| {
        let mut len = 0;
        for &s in black_box(STRINGS) {
            len += Tmpl(s).to_string().len();
        }
        len
    });
}

fn escape_json_pretty(b: &mut criterion::Bencher<'_>) {
    #[derive(Template)]
    #[template(ext = "html", source = "{{self.0|json(2)|safe}}")]
    struct Tmpl(&'static str);

    b.iter(|| {
        let mut len = 0;
        for &s in black_box(STRINGS) {
            len += Tmpl(s).to_string().len();
        }
        len
    });
}

fn escape_json_for_html(b: &mut criterion::Bencher<'_>) {
    #[derive(Template)]
    #[template(ext = "html", source = "{{self.0|json}}")]
    struct Tmpl(&'static str);

    b.iter(|| {
        let mut len = 0;
        for &s in black_box(STRINGS) {
            len += Tmpl(s).to_string().len();
        }
        len
    });
}

fn escape_json_for_html_pretty(b: &mut criterion::Bencher<'_>) {
    #[derive(Template)]
    #[template(ext = "html", source = "{{self.0|json(2)}}")]
    struct Tmpl(&'static str);

    b.iter(|| {
        let mut len = 0;
        for &s in black_box(STRINGS) {
            len += Tmpl(s).to_string().len();
        }
        len
    });
}

const STRINGS: &[&str] = include!("strings.inc");
