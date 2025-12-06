# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./examples/vhs/hello-world.tape`
Output "target/hello-world.gif"
Set Theme "Aardvark Blue"
Set Width 1200
Set Height 200
Hide
Type "cargo run -p hello-world"
Enter
Sleep 1s
Show
Sleep 5s
