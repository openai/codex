# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./examples/vhs/scrollbar.tape`
Output "target/scrollbar.gif"
Set Theme "Aardvark Blue"
Set Width 1200
Set Height 1200
Hide
Type "cargo run -p scrollbar"
Enter
Sleep 1s
Show
Sleep 5s
