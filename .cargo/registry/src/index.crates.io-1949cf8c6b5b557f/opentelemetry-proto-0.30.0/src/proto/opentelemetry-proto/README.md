# OpenTelemetry Protocol (OTLP) Specification

[![Build Check](https://github.com/open-telemetry/opentelemetry-proto/workflows/Build%20Check/badge.svg?branch=main)](https://github.com/open-telemetry/opentelemetry-proto/actions?query=workflow%3A%22Build+Check%22+branch%3Amain)

This repository contains the [OTLP protocol specification](docs/specification.md)
and the corresponding Language Independent Interface Types ([.proto files](opentelemetry/proto)).

## Language Independent Interface Types

The proto files can be consumed as GIT submodules or copied and built directly in the consumer project.

The compiled files are published to central repositories (Maven, ...) from OpenTelemetry client libraries.

See [contribution guidelines](CONTRIBUTING.md) if you would like to make any changes.

## OTLP/JSON

See additional requirements for [OTLP/JSON wire representation here](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/protocol/otlp.md#json-protobuf-encoding).

## Generate gRPC Client Libraries

To generate the raw gRPC client libraries, use `make gen-${LANGUAGE}`. Currently supported languages are:

* cpp
* csharp
* go
* java
* objc
* openapi (swagger)
* php
* python
* ruby

## Maturity Level

1.0.0 and newer releases from this repository may contain unstable (alpha or beta)
components as indicated by the Maturity table below.

| Component | Binary Protobuf Maturity | JSON Maturity |
| --------- |--------------- | ------------- |
| common/* | Stable | [Stable](docs/specification.md#json-protobuf-encoding) |
| resource/* | Stable | [Stable](docs/specification.md#json-protobuf-encoding) |
| metrics/\*<br>collector/metrics/* | Stable | [Stable](docs/specification.md#json-protobuf-encoding) |
| trace/\*<br>collector/trace/* | Stable | [Stable](docs/specification.md#json-protobuf-encoding) |
| logs/\*<br>collector/logs/* | Stable | [Stable](docs/specification.md#json-protobuf-encoding) |
| profiles/\*<br>collector/profiles/* | Development | [Development](docs/specification.md#json-protobuf-encoding) |

(See [Versioning and Stability](https://github.com/open-telemetry/opentelemetry-specification/blob/a08d1f92f62acd4aafe4dfaa04ae7bf28600d49e/specification/versioning-and-stability.md)
for definition of maturity levels).

## Stability Definition

Components marked `Stable` provide the following guarantees:

- Field types, numbers and names will not change.
- Service names and `service` package names will not change.
- Service method names will not change. [from 1.0.0]
- Service method parameter names will not change. [from 1.0.0]
- Service method parameter types and return types will not change. [from 1.0.0]
- Service method kind (unary vs streaming) will not change.
- Names of `message`s and `enum`s will not change. [from 1.0.0]
- Numbers assigned to `enum` choices will not change.
- Names of `enum` choices will not change. [from 1.0.0]
- The location of `message`s and `enum`s, i.e. whether they are declared at the top lexical
  scope or nested inside another `message` will not change. [from 1.0.0]
- Package names and directory structure will not change. [from 1.0.0]
- `optional` and `repeated` declarators of existing fields will not change. [from 1.0.0]
- No existing symbol will be deleted.  [from 1.0.0]

Note: guarantees marked [from 1.0.0] will go into effect when this repository is tagged
with version number 1.0.0.

The following additive changes are allowed:

- Adding new fields to existing `message`s.
- Adding new `message`s or `enum`s.
- Adding new choices to existing `enum`s.
- Adding new choices to existing `oneof` fields.
- Adding new `service`s.
- Adding new `method`s to existing `service`s.

All the additive changes above must be accompanied by an explanation about how
new and old senders and receivers that implement the version of the protocol
before and after the change interoperate.

## Experiments

### New Experimental Components  

Sometimes we need to experiment with new components, for example to add a
completely new signal to OpenTelemetry. In this case, to define new experimental
components we recommend placing new proto files in a "development" sub-directory.
Such isolated experimental components are excluded from
above [stability requirements](#stability-definition).

We recommend using
`Development`, `Alpha`, `Beta`, `Release Candidate`
[levels](https://github.com/open-telemetry/opentelemetry-specification/blob/main/oteps/0232-maturity-of-otel.md#maturity-levels)
to communicate different grades of readiness of new components.

Experimental components may be removed completely at the end of the experiment,
provided that they are not referenced from any `Stable` component.

Experiments which succeed, require a review to be marked `Stable`. Once marked
`Stable` they become subject to the [stability requirements](#stability-definition).

### Experimental Additions to Stable Components

New experimental fields or messages may be added in `Development` state to `Stable`
components. The experimental fields and messages within `Stable components` are subject
to the full [stability requirements](#stability-definition), and in addition, they must be
clearly labeled as `Development` (or as any other non-`Stable` level) in the .proto file
source code.

If an experiment concludes and the previously added field or message is not needed
anymore, the field/message must stay, but it may be declared "deprecated". During all
phases of experimentation it must be clearly specified that the field or message may be
deprecated. Typically, deprecated fields are left empty by the senders and the recipients
that participate in experiments must expect during all experimental phases (including
_after_ the experiment is concluded) that the experimental field or message has an
empty value.

Experiments which succeed, require a review before the field or the message is marked
`Stable`.

## Generated Code

No guarantees are provided whatsoever about the stability of the code that
is generated from the .proto files by any particular code generator.
