# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./examples/vhs/demo2-social.tape`

Output "target/demo2-social.gif"
Set Theme "Aardvark Blue"
# Github social preview size (1280x640 with 80px padding) and must be < 1MB
# This puts some constraints on the amount of interactivity we can do.
Set Width 1280
Set Height 640
Set Padding 80
Hide
Type "cargo run -p demo2"
Enter
Sleep 2s
Show
# About screen
Sleep 3.5s
Down # Red eye
Sleep 0.4s
Down # black eye
Sleep 1s
Tab
# Recipe
Sleep 1s
Set TypingSpeed 500ms
Down 7
Sleep 1s
Tab
# Email
Sleep 2s
Down 4
Sleep 2s
Tab
# Trace route
Sleep 1s
Set TypingSpeed 200ms
Down 10
Sleep 2s
Tab
# Weather
Set TypingSpeed 100ms
Down 40
Sleep 2s
