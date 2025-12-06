# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./examples/vhs/table.tape`
Output "target/table.gif"
Set Theme "Aardvark Blue"
Set Width 1400
Set Height 768
Hide
Type "cargo run -p table"
Enter
Sleep 1s
Show
Sleep 2s
Set TypingSpeed 1s
Down 3
Up 6
Sleep 1s
Down 3
Sleep 1s
Right 1
Sleep 1s
Right 1
Sleep 1s
Right 1
Sleep 1s
Right 1
Sleep 2s
