# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./examples/vhs/panic.tape`
Output "target/panic.gif"
Set Theme "Aardvark Blue"
Set Width 1200
Set Height 600
Type "cargo run -p panic"
Enter
Sleep 5s
Type p
Sleep 2s
Type reset
Enter
Type "cargo run -p panic"
Enter
Sleep 2s
Type e
Sleep 2s
Type p
Sleep 5s
