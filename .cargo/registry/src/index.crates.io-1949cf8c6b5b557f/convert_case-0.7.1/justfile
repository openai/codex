test *FILTER:
    cargo test {{FILTER}}
    cargo test --features random

watch-test:
    watchexec -e rs -rc reset -- just test

build:
    cargo build --all

watch-build:
    watchexec -- "reset && just build"

doc:
    cargo doc --all-features

watch-doc:
    watchexec -- "just doc && cargo test --all-features --doc"

tree:
    tree -I target
