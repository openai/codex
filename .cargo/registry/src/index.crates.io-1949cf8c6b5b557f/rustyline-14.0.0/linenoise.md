Mapping between linenoise API and rustyline API

| linenoise                      | rustyline                    | Remarks                   |
|--------------------------------|------------------------------|---------------------------|
| linenoiseState                 | State                        |                           |
| *Blocking API*                 |
| linenoise                      | Editor::readline             |
| linenoiseFree                  | _                            | RAII                      |
| *Non blocking API*             |                              |
| linenoiseEditStart             | _                            |
| linenoiseEditFeed              | _                            |
| linenoiseEditStop              | _                            |
| linenoiseHide                  | Renderer::clear_rows         |
| linenoiseShow                  | State::refresh_line          |
| *Completion API*               |
| linenoiseCompletions           | Vec<Candidate>               |
| linenoiseCompletionCallback    | Completer                    |
| linenoiseAddCompletion         | _                            | std Vec::add              |
| linenoiseSetCompletionCallback | Editor::set_helper           |
| linenoiseHintsCallback         | Hinter                       |
| linenoiseSetHintsCallback      | Editor::set_helper           |
| linenoiseFreeHintsCallback     | _                            | RAII                      |
| linenoiseSetFreeHintsCallback  | _                            | RAII                      |
| *History API*                  |
| linenoiseHistoryAdd            | Editor::add_history_entry    |
| linenoiseHistorySetMaxLen      | Editor::set_max_history_size |
| linenoiseHistorySave           | Editor::save_history         |
| linenoiseHistoryLoad           | Editor::load_history         |
| *Other utilities*              |
| linenoiseClearScreen           | Editor::clear_screen         |
| linenoiseSetMultiLine          | _                            | Always activated          |
| linenoisePrintKeyCodes         | _                            | debug logs                |
| linenoiseMaskModeEnable        | _                            | see read_password example |
| linenoiseMaskModeDisable       | _                            |
