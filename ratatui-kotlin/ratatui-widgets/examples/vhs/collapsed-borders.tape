# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./ratatui-widgets/examples/vhs/collapsed-borders.tape`
Output "target/collapsed-borders.gif"
Set Theme "Aardvark Blue"
Set Width 1200
Set Height 800
Hide
Type "cargo run -p ratatui-widgets --example collapsed-borders"
Enter
Sleep 2s
Show
Sleep 1s
Left Sleep 1s
Right Sleep 1s
Down Sleep 1s
Hide
Type "q"
