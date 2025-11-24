#[test]
#[cfg(unix)]
fn run() {
    use lsp_types::lsif::Entry;

    let jsonl = include_str!("tsc-unix.lsif");
    for json in jsonl.lines() {
        let r = serde_json::from_str::<Entry>(&json).expect(&format!("can not parse {}", json));
        let x = serde_json::to_string(&r).expect(&format!("can not serialize {}", json));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&x).unwrap(),
            serde_json::from_str::<serde_json::Value>(json).unwrap(),
            "and strings:\ntheir: {}\n  our: {}",
            json,
            x,
        );
    }
}
