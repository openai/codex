# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./examples/vhs/async-github.tape`
Output "target/async-github.gif"
Set Theme "Aardvark Blue"
Set Width 1200
Set Height 600
Hide
Type "cargo run -p async-github"
Enter
Sleep 1s
Show
Sleep 1s
Set TypingSpeed 0.5s
Down 3
Up
Sleep 1s
