# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./examples/vhs/constraints.tape`
Output "target/constraints.gif"
Set Theme "Aardvark Blue"
Set FontSize 18
Set Width 1200
Set Height 700
Hide
Type "cargo run -p constraints"
Enter
Sleep 2s
Show
Sleep 5s
Right @5s 7
