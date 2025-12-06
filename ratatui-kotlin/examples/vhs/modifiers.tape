# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./examples/vhs/modifiers.tape`
Output "target/modifiers.gif"
Set Theme "Aardvark Blue"
Set Width 1200
Set Height 1460
Hide
Type "cargo run -p modifiers"
Enter
Sleep 2s
Show
Sleep 1s
