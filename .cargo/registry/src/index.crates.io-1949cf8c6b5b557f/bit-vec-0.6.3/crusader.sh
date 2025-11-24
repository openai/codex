#!/bin/bash

git clone https://github.com/brson/cargo-crusader
cd cargo-crusader
cargo build --release
export PATH=$PATH:`pwd`/target/release/
cd ../

cargo crusader

exit
