# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./examples/vhs/custom-widget.tape`
Output "target/custom-widget.gif"
Set Theme "Aardvark Blue"
Set Width 760
Set Height 260
Hide
Type "cargo run -p custom-widget"
Enter
Sleep 2s
Show
Sleep 1s
Set TypingSpeed 0.7s
Right
Right
Space
Space
Left
Space
Left
Space
