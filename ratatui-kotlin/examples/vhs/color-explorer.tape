# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./examples/vhs/color-explorer.tape`
Output "target/color-explorer.gif"
Set Theme "Aardvark Blue"
Set Width 1200
Set Height 1410
Hide
Type "cargo run -p color-explorer"
Enter
Sleep 2s
Show
Sleep 1s
