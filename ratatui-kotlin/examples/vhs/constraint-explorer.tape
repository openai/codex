# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./examples/vhs/constraint-explorer.tape`
Output "target/constraint-explorer.gif"
Set Theme "Aardvark Blue"
Set FontSize 18
Set Width 1200
Set Height 950
Hide
Type "cargo run -p constraint-explorer"
Enter
Sleep 2s
Show
Set TypingSpeed 2s
Type "1"
Type "2"
Right
Type "4"
Type "5"
Up
Up
Down
Down
Right
Set TypingSpeed 0.5s
Type "++++++++"
Type "--------"
Type "aaa"
Sleep 2s
Type "xxx"
Hide
