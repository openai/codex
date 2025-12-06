# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./ratatui-widgets/examples/vhs/table.tape`
Output "target/table.gif"
Set Theme "Aardvark Blue"
Set Width 1400
Set Height 768
Hide
Type "cargo run -p ratatui-widgets --example table"
Enter
Sleep 1s
Show
Sleep 2s
Set TypingSpeed 1s
Down 3
Sleep 1s
Right 3
Sleep 2s
