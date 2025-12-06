# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./examples/vhs/chart.tape`
Output "target/chart.gif"
Set Theme "Aardvark Blue"
Set Width 1200
Set Height 800
Hide
Type "cargo run -p chart"
Enter
Sleep 1s
Show
Sleep 5s
