# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./examples/vhs/tracing.tape`
Output "target/tracing.gif"
Set Theme "Aardvark Blue"
Set Width 1200
Set Height 800
Type"RUST_LOG=trace cargo run -p tracing" Enter
Sleep 1s
Type @100ms "jjjjq"
Sleep 1s
Type "cat tracing.log" Enter
Sleep 10s
