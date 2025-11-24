#![cfg(all(feature = "env-filter", feature = "fmt"))]
use tracing::{self, subscriber::with_default, Span};
use tracing_subscriber::{filter::EnvFilter, FmtSubscriber};

#[test]
fn duplicate_spans() {
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::new("[root]=debug"))
        .finish();

    with_default(subscriber, || {
        let root = tracing::debug_span!("root");
        root.in_scope(|| {
            // root:
            assert_eq!(root, Span::current(), "Current span must be 'root'");
            let leaf = tracing::debug_span!("leaf");
            leaf.in_scope(|| {
                // root:leaf:
                assert_eq!(leaf, Span::current(), "Current span must be 'leaf'");
                root.in_scope(|| {
                    // root:leaf:
                    assert_eq!(
                        leaf,
                        Span::current(),
                        "Current span must be 'leaf' after entering twice the 'root' span"
                    );
                })
            });
            // root:
            assert_eq!(
                root,
                Span::current(),
                "Current span must be root ('leaf' exited, nested 'root' exited)"
            );

            root.in_scope(|| {
                assert_eq!(root, Span::current(), "Current span must be root");
            });
            // root:
            assert_eq!(
                root,
                Span::current(),
                "Current span must still be root after exiting nested 'root'"
            );
        });
    });
}
