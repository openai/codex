# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./ratatui-widgets/examples/vhs/gauge.tape`
Output "target/gauge.gif"
Set Theme "Aardvark Blue"
Set Width 1200
Set Height 850
Hide
Type "cargo run -p ratatui-widgets --example gauge"
Enter
Sleep 2s
Show
Sleep 5s
