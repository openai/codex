# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./examples/vhs/todo-list.tape`
Output "target/todo-list.gif"
Set Theme "Aardvark Blue"
Set Width 1200
Set Height 612
Hide
Type "cargo run -p todo-list"
Enter
Sleep 10s
Show
Sleep 2s
Down@1.5s 3
Right@1.5s 1
Sleep 1.5s
Down@1.5s 3
Sleep 1.5s
Up@1s 1
Sleep 1s
Right@1.5s 1
Sleep 1.5s
Up@1s 4
Left@1s 1
Sleep 2s
