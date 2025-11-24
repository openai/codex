# Design Goals for OpenTelemetry Wire Protocol

We want to design a telemetry data exchange protocol that has the following characteristics:

- Be suitable for use between all of the following node types: instrumented applications, telemetry backends, local agents, stand-alone collectors/forwarders.

- Have high reliability of data delivery and clear visibility when the data cannot be delivered.

- Have low CPU usage for serialization and deserialization.

- Impose minimal pressure on memory manager, including pass-through scenarios, where deserialized data is short-lived and must be serialized as-is shortly after and where such short-lived data is created and discarded at high frequency (think telemetry data forwarders).

- Support ability to efficiently modify deserialized data and serialize again to pass further. This is related but slightly different from the previous requirement.

- Ensure high throughput (within the available bandwidth) in high latency networks (e.g. scenarios where telemetry source and the backend are separated by high latency network).

- Allow backpressure signalling.

- Be load-balancer friendly (do not hinder re-balancing).
