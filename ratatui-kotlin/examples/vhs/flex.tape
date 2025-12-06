# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./examples/vhs/flex.tape`
Output "target/flex.gif"
Set Theme "Aardvark Blue"
Set Width 1200
Set Height 1410
Hide
Type "cargo run -p flex"
Enter
Sleep 2s
Show
Sleep 2s
Right @5s 7
Sleep 2s
Left 7
Sleep 2s
Down @200ms 50
