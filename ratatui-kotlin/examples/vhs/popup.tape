# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./examples/vhs/popup.tape`
Output "target/popup.gif"
Set Theme "Aardvark Blue"
Set Width 1200
Set Height 600
Hide
Type "cargo run -p popup"
Enter
Sleep 1s
Show
Sleep 2s
Type p
Sleep 2s
Type p
Sleep 5s
