# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./ratatui-widgets/examples/vhs/logo.tape`
Output "target/logo.gif"
Set Theme "Aardvark Blue"
Set Width 550
Set Height 220
Hide
Type "cargo run -p ratatui-widgets --example logo"
Enter
Sleep 2s
Show
Sleep 2s
Hide
Escape
