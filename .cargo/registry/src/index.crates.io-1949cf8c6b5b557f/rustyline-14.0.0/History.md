# Config

max_history_size

# Current session

- we should remember (index) of the first line inserted by this session.
  - if no line has been inserted => do nothing on save
  - reset this index after saving successfully.
- we should remember (path and timestamp) of the file used to initialize/`load` history.
  - if path used to save history is the same:
    - if timestamp is still the same => we can append only new lines if history has not been truncated.
    - update timestamp after saving successfully.
- we should remember (path and timestamp) of the file used to persist/`save` history.
  - reset them if `load` is then called with a different path
  - update them if `load` is then called with same path.
  - update them after saving successfully
  - if path used to save history is the same:
    - if timestamp is still the same => we can append only new lines if history has not been truncated.

```
HistoryInfo
  first_add_index: Option<usize>, // first line inserted by this session
  truncated: bool //
  path_info: Option<PathInfo>,
```

```
PathInfo
  path: Path,
  modified: SystemTime,
```
