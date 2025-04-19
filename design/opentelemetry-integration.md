# OpenTelemetry Integration for Codex CLI

## Overview
Add OpenTelemetry support to Codex CLI to enable observability for agent operations, API calls, command execution, and performance metrics.

## Background
OpenTelemetry is an open-source observability framework that provides vendor-neutral APIs, libraries, agents, and instrumentation to enable collection of distributed traces and metrics. Integrating OpenTelemetry into Codex would enable:

1. Performance monitoring of agent operations
2. Tracing of complex flows (e.g., command execution chains)
3. Error analysis and debugging
4. Usage pattern insights
5. Integration with existing monitoring systems

## Scope and Requirements

### Core Components
- **Tracing:** Capture spans for key operations (API calls, command execution, file operations)
- **Metrics:** Track performance data (response times, token usage, command success rates)
- **Context Propagation:** Maintain context across async operations
- **Exporters:** Support multiple backends (console, OTLP, Jaeger, etc.)
- **Environment Configuration:** Simple configuration via env vars or config file

### Technical Requirements
1. Minimal performance impact
2. No breaking changes to existing API
3. Optional activation (disabled by default)
4. Compatible with all platforms supported by Codex
5. Safe handling of sensitive data

## Implementation Roadmap

### Milestone 1: Local Collector with Minimal Complexity
The initial implementation focuses on simplicity and user-friendliness with minimal repository complexity:

1. **User-Local Configuration Approach**
   - Store configuration in `~/.codex/otel-config.yaml`
   - Use file-based exporters for easy inspection
   - Configure through environment variables

2. **Core Implementation**
   ```typescript
   // src/utils/telemetry/otel.ts - Basic SDK setup
   import { NodeSDK } from '@opentelemetry/sdk-node';
   import { OTLPTraceExporter } from '@opentelemetry/exporter-trace-otlp-http';
   import { Resource } from '@opentelemetry/resources';
   import { SemanticResourceAttributes } from '@opentelemetry/semantic-conventions';
   import { existsSync } from 'fs';
   import { homedir } from 'os';
   import path from 'path';

   // Only initialize if user has opted in
   const configPath = path.join(homedir(), '.codex', 'otel-config.yaml');
   const isEnabled = process.env.CODEX_OTEL_ENABLED === 'true' || existsSync(configPath);

   export const initTelemetry = () => {
     if (!isEnabled) return null;
     
     const sdk = new NodeSDK({
       resource: new Resource({
         [SemanticResourceAttributes.SERVICE_NAME]: 'codex-cli',
         [SemanticResourceAttributes.SERVICE_VERSION]: process.env.npm_package_version || 'unknown',
       }),
       traceExporter: new OTLPTraceExporter({
         url: process.env.CODEX_OTEL_ENDPOINT || 'http://localhost:4318/v1/traces',
       }),
     });
     
     sdk.start();
     return sdk;
   };
   ```

3. **Sample Local Collector Configuration**
   ```yaml
   # ~/.codex/otel-config.yaml
   receivers:
     otlp:
       protocols:
         http:
           endpoint: localhost:4318

   processors:
     batch:
       timeout: 1s

   exporters:
     file:
       path: ~/.codex/telemetry/codex-traces.json
     logging:
       verbosity: detailed

   service:
     pipelines:
       traces:
         receivers: [otlp]
         processors: [batch]
         exporters: [file, logging]
   ```

4. **Instrumentation Wrapper for Key Points**
   ```typescript
   // src/utils/telemetry/tracing.ts
   import { trace, context, SpanStatusCode } from '@opentelemetry/api';

   const tracer = trace.getTracer('codex-cli');

   export const tracedOperation = async (name, operation, metadata = {}) => {
     if (!process.env.CODEX_OTEL_ENABLED) {
       return operation();
     }
     
     const span = tracer.startSpan(name);
     Object.entries(metadata).forEach(([key, value]) => {
       span.setAttribute(key, value);
     });
     
     try {
       const result = await operation();
       span.end();
       return result;
     } catch (error) {
       span.setStatus({
         code: SpanStatusCode.ERROR,
         message: error.message,
       });
       span.end();
       throw error;
     }
   };
   ```

5. **Initial Instrumentation Points**
   - OpenAI API calls
   - Command execution
   - User approvals/rejections

6. **Demo/Testing Process**
   - Run local OpenTelemetry Collector: `otelcol --config=~/.codex/otel-config.yaml`
   - Start Codex with telemetry: `CODEX_OTEL_ENABLED=true codex`
   - View traces in `~/.codex/telemetry/codex-traces.json`

7. **Documentation**
   - Add setup instructions in README
   - Include sample configuration
   - Document environment variables

### Future Milestones

#### Milestone 2: Enhanced Instrumentation
- Add comprehensive metrics collection
- Implement auto-instrumentation for popular frameworks
- Add custom context propagation
- Create dashboards for visualization

#### Milestone 3: Cloud-Ready Integrations
- Add support for popular cloud observability platforms
- Implement sampling strategies for high-volume scenarios
- Create aggregation views for multi-user deployments

## Implementation Details

### Dependencies
- `@opentelemetry/sdk-node`
- `@opentelemetry/auto-instrumentations-node`
- `@opentelemetry/exporter-trace-otlp-http` (or other exporters)
- `@opentelemetry/resources`
- `@opentelemetry/semantic-conventions`

### Key Instrumentation Points
1. **Agent Loop:**
   - Track overall conversation cycles
   - Measure thinking time vs. execution time

2. **API Integration:**
   - Track API calls to OpenAI
   - Measure latency, token usage, and error rates

3. **Command Execution:**
   - Track command lifecycle (approval, execution, results)
   - Measure execution time and success/failure rates

4. **File Operations:**
   - Track file reads/writes
   - Measure sizes and operation times

5. **User Interactions:**
   - Track approvals/rejections
   - Measure response times

### Configuration Options
- `CODEX_OTEL_ENABLED`: Enable/disable (bool, default: false)
- `CODEX_OTEL_SERVICE_NAME`: Service name for traces (string, default: "codex-cli")
- `CODEX_OTEL_EXPORTER`: Exporter type (console, otlp, jaeger, etc.)
- `CODEX_OTEL_ENDPOINT`: Collector endpoint URL
- `CODEX_OTEL_HEADERS`: Additional headers for OTLP export
- `CODEX_OTEL_SAMPLE_RATE`: Sampling rate (0.0-1.0, default: 1.0)

### Security Considerations
- Sanitize sensitive data (API keys, file contents) from traces
- Provide control over what data gets logged
- Anonymous session IDs by default

## Testing Plan
- Unit tests for instrumentation code
- Integration tests with mock exporters
- Performance comparison tests (with/without telemetry)
- Dashboard examples for common metrics

## Documentation
- Add configuration docs to README
- Create example dashboards for popular backends
- Document common metrics and their interpretations

## Success Criteria
- Zero breaking changes to existing functionality
- <3% performance overhead when enabled
- Successfully captures all critical paths in the application
- Data exports properly to supported backends
- Documentation and examples for users to implement

## Expected Benefits
- Improved debugging capabilities
- Better understanding of performance bottlenecks
- Insights into user interaction patterns
- Foundation for future performance improvements