# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./ratatui-widgets/examples/vhs/block.tape`
Output "target/block.gif"
Set Theme "Aardvark Blue"
Set Width 1200
Set Height 1200
Hide
Type "cargo run -p ratatui-widgets --example block"
Enter
Sleep 2s
Show
Sleep 2s
