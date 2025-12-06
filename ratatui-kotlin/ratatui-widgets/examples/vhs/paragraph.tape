# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./ratatui-widgets/examples/vhs/paragraph.tape`
Output "target/paragraph.gif"
Set Theme "Aardvark Blue"
Set Width 1200
Set Height 600
Hide
Type "cargo run -p ratatui-widgets --example paragraph"
Enter
Sleep 1s
Show
Sleep 5s
