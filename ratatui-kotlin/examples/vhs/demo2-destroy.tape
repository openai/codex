# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./examples/vhs/demo2-destroy.tape`
# NOTE: Requires VHS 0.6.1 or later for Screenshot support
Output "target/demo2-destroy.gif"
Set Theme "Aardvark Blue"
# The reason for this strange size is that the social preview image for this
# demo is 1280x64 with 80 pixels of padding on each side. We want a version
# without the padding for README.md, etc.
Set Width 1120
Set Height 480
Set Padding 0
Hide
Type "cargo run -p demo2"
Enter
Sleep 2s
Show
Type "d"
Sleep 20s
