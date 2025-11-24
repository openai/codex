# OTLP JSON request examples

This folder contains a collection of example OTLP JSON files for all signals
that can be used as request payloads.

- Trace [trace.json](trace.json)
- Metrics [metrics.json](metrics.json)
- Logs [logs.json](logs.json)
  - Events [events.json](events.json)

## Trying it out

First run a OpenTelemetry collector with the following configuration:

```yaml
receivers:
  otlp:
    protocols:
      http:

exporters:
  debug:
    verbosity: detailed

service:
  pipelines:
    traces:
      receivers: [otlp]
      exporters: [debug]
    metrics:
      receivers: [otlp]
      exporters: [debug]
    logs:
      receivers: [otlp]
      exporters: [debug]
```

Then send a curl request to the collector (e.g. for Logs):

```shell
curl -X POST -H "Content-Type: application/json" -d @logs.json -i localhost:4318/v1/logs
```

> Remember to change the URL path when sending other signals (traces/metrics).
