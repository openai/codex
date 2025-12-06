# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./ratatui-widgets/examples/vhs/canvas.tape`
Output "target/canvas.gif"
Set Theme "Aardvark Blue"
Set FontSize 12
Set Width 1200
Set Height 800
Hide
Type "cargo run -p ratatui-widgets --example canvas"
Enter
Sleep 2s
Show
Sleep 5s
