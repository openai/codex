# Extra features

| Alt            | Scroll | Continuation prompt | Right prompt | Suspend |
| -------------- | ------ | ------------------- | ------------ | ------- |
| isocline       | X      | X                   |              |         |
| linefeed       |        |                     |              | Unix    |
| liner          |        |                     |              |         |
| prompt-toolkit | X      | X                   | X            | Unix    |
| reedline       |        | X                   | X            |         |
| replxx         |        |                     |              | Unix    |
| rustyline      |        |                     |              | Unix    |
| termwiz        |        |                     |              |         |

Scroll: for very long line (longer than screen cols\*rows), scrolls from start to end.\
Continuation prompt: for multiline input, display a different prompt\
Suspend: Control-Z

| Alt            | Editable History | Custom history backend | History timestamp |
| -------------- | ---------------- | ---------------------- | ----------------- |
| isocline       |                  |                        |                   |
| linefeed       | X                |                        |                   |
| liner          |                  |                        |                   |
| prompt-toolkit |                  | X                      |                   |
| reedline       |                  | X                      |                   |
| replxx         | X                |                        | X                 |
| rustyline      |                  | X                      |                   |
| termwiz        |                  | X                      | \*                |

Editable History: any history entry can be edited and saved\
Custom history backend: history persistence can be customized\
History timestamp: history entries are timestamped

Mouse support

Text selection

Completion candidates display

Multiple commands for a keybinding

Auto indent

Minimal repaint

Overwrite mode

Lexer / Parser

Configuration file (inputrc)

Dynamic prompt (editing mode)

External print
