# Output

| Seq       | Terminfo                   | Comment                                                  |
| --------- | -------------------------- | -------------------------------------------------------- |
| \E[H      | cursor_home, home, ho      |                                                          |
| \E[K      | clr_eol, el, ce            |                                                          |
| \E[H\E[J  | clear_screen, clear, cl    |                                                          |
| \E[6n     | user7, u7, u7              | cursor position report                                   |
| ^M        | carriage_return, cr, cr    | move cursor to bol                                       |
| \E[B      | cursor_down, cud1, do      | ^J                                                       |
| \E[%p1%dB | parm_down_cursor, cud, DO  |                                                          |
| \E[A      | cursor_up, cuu1, up        |                                                          |
| \E[%p1%dA | parm_up_cursor, cuu, UP    |                                                          |
| \E[C      | cursor_right, cuf1, nd     |                                                          |
| \E[%p1%dC | parm_right_cursor, cuf, RI |                                                          |
| \E[D      | cursor_left, cub1, le      | ^H                                                       |
| \E[%p1%dD | parm_left_cursor, cub, LE  |                                                          |
| ^G        | bell, bel, bl              |                                                          |
| \E[?2004h |                            | bracketed paste on                                       |
| \E[?2004l |                            | bracketed paste off                                      |
| \E[?1000h |                            | X11 mouse reporting, reports on button press and release |
| \E[?1015h |                            | Enable urxvt Mouse mode                                  |
| \E[?1006h |                            | Enable Xterm SGR mouse mode                              |
