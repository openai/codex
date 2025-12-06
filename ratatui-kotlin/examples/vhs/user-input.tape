# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./examples/vhs/user-input.tape`
Output "target/user-input.gif"
Set Theme "Aardvark Blue"
Set Width 1200
Set Height 600
Hide
Type "cargo run -p user-input"
Enter
Sleep 1s
Show
Sleep 2s
Type e
Sleep 1s
Type "Hello, world!"
Enter
Sleep 2s
Backspace 13
Sleep 1s
Type "Goodbye, world!"
Enter
Sleep 5s
