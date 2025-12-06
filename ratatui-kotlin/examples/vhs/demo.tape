# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./examples/vhs/demo.tape`
Output "target/demo.gif"
Set Theme "Aardvark Blue"
Set Width 1200
Set Height 1200
Set PlaybackSpeed 0.5
Hide
Type "cargo run -p demo"
Enter
Sleep 2s
Show
Sleep 1s
Down@1s 12
Right
Sleep 4s
Right
Sleep 4s
