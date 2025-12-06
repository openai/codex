# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./examples/vhs/advanced-widget-impl.tape`
Output "target/advanced-widget-impl.gif"
Set Theme "Aardvark Blue"
Set Width 760
Set Height 260
Hide
Type "cargo run -p advanced-widget-impl"
Enter
Sleep 2s
Show
Sleep 5s
