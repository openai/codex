## Summary
- inject the current local date into `EnvironmentContext` so every turn shares an ISO8601 `<current_date>` tag with the model
- extend the serializer/equality helpers to account for the new field while keeping comparisons deterministic in tests
- add lightweight unit coverage to lock the XML output and default date format

## Testing
- ./build-fast.sh

## Acceptance Criteria
- environment context payloads now surface a `<current_date>` element in YYYY-MM-DD form
- existing comparisons that ignore shell differences remain stable once the date is normalized
- unit tests document the new field and its formatting so regressions are caught automatically
