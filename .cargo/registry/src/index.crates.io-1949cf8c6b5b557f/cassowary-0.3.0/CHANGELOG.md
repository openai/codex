# 0.3

* Various fixes (PR #4) from @christolliday. 
  Main breaking change is that variables no longer silently initialise to zero and will report
  their initial value in the first call to `fetch_changes`, also `has_edit_variable` now takes `&self`
  instead of `&mut self`.

## 0.2.1

* Fixed crash under certain use cases. See PR #1 (Thanks @christolliday!).

# 0.2.0

* Changed API to only report changes to the values of variables. This allows for more efficient use of the library in typical applications.

# 0.1

Initial release
