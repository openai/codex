# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./examples/vhs/hyperlink.tape`
Output "target/hyperlink.gif"
Set Theme "Aardvark Blue"
Set Width 1200
Set Height 200
Hide
Type "cargo run -p hyperlink"
Enter
Sleep 3s
Show
Sleep 1s
Hide
Type "q"

