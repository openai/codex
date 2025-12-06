# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./ratatui-widgets/examples/vhs/tabs.tape`
Output "target/tabs.gif"
Set Theme "Aardvark Blue"
Set Width 1200
Set Height 368
Hide
Type "cargo run -p ratatui-widgets --example tabs"
Enter
Sleep 2s
Show
Sleep 1s
Right@2.5s 3
Left@2.5s 3
Sleep 2s
