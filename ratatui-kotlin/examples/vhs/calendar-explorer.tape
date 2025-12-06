# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./examples/vhs/calendar-explorer.tape`
Output "target/calendar-explorer.gif"
Set Theme "Aardvark Blue"
Set Width 1200
Set Height 800
Hide
Type "cargo run -p calendar-explorer"
Enter
Sleep 3s
Show
Set TypingSpeed 1s
Type "s"
Type "s"
Type "s"
Type 'n'
Type 'p'
Down 5
Right 5
