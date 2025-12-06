# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./examples/vhs/canvas.tape`
Output "target/canvas.gif"
Set Theme "Aardvark Blue"
Set FontSize 12
Set Width 1200
Set Height 800
Hide
Type "cargo run -p canvas"
Enter
Sleep 2s
Show
Sleep 30s
