# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs ./examples/vhs/release-header.tape`
# NOTE: Requires VHS 0.6.1 or later for Screenshot support
# The reason for this strange size is that the social preview image for this
# demo is 1280x64 with 80 pixels of padding on each side. We want a version
# without the padding for README.md, etc.
Set Theme {"background": "#141432"}
Set FontSize 25
Set Width 1120
Set Height 480
Set Padding 0
Hide
Type "cargo run -p release-header"
Enter
Sleep 1s
Show
Sleep 1s
Screenshot assets/release-header.png
Sleep 1s
