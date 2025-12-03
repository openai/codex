## Summary

Refactor of the `execpolicy` crate

To make `1` possible, we needed to refactor the `execpolicy` crate. To illustrate why, consider an agent attempting to run `apple | rm -rf ./`. Suppose `apple` is allowed by `execpolicy`. Before this PR, `execpolicy` would consider `apple` and `pear` and only render one rule match: `Allow`. We would skip any heuristics checks on `rm -rf ./` and immediately approve `apple | rm -rf ./` to run.

To fix this, we now thread a `fallback` evaluation function into `execpolicy` that runs when no `execpolicy` rules match a given command. In our example, we would run `fallback` on `rm -rf ./` and prevent `apple | rm -rf ./` from being run without approval.

## Testing

- not run (not requested)
