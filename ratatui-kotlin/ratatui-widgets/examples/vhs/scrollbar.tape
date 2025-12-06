# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./ratatui-widgets/examples/vhs/scrollbar.tape`
Output "target/scrollbar.gif"
Set Theme "Aardvark Blue"
Set Width 1200
Set Height 600
Hide
Type "cargo run -p ratatui-widgets --example scrollbar"
Enter
Sleep 1s
Show
Sleep 2s
Right@1s 4
Sleep 1s
Left@1s 1
Down@1s 3
Sleep 2s
