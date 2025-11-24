# Rust Landlock library

Landlock is a security feature available since Linux 5.13.
The goal is to enable to restrict ambient rights (e.g., global filesystem access) for a set of processes by creating safe security sandboxes as new security layers in addition to the existing system-wide access-controls.
This kind of sandbox is expected to help mitigate the security impact of bugs, unexpected or malicious behaviors in applications.
Landlock empowers any process, including unprivileged ones, to securely restrict themselves.
More information about Landlock can be found in the [official website](https://landlock.io).

This Rust crate provides a safe abstraction for the Landlock system calls along with some helpers.

## Use cases

This crate is especially useful to protect users' data by sandboxing:
* trusted applications dealing with potentially malicious data
  (e.g., complex file format, network request) that could exploit security vulnerabilities;
* sandbox managers, container runtimes or shells launching untrusted applications.

## Examples

A simple example can be found with the
[`path_beneath_rules()`](https://landlock.io/rust-landlock/landlock/fn.path_beneath_rules.html) helper.
More complex examples can be found with the
[`Ruleset` documentation](https://landlock.io/rust-landlock/landlock/struct.Ruleset.html)
and the [sandboxer example](examples/sandboxer.rs).

## [Crate documentation](https://landlock.io/rust-landlock/landlock/)

## Changelog

* [v0.4.2](CHANGELOG.md#v042)
* [v0.4.1](CHANGELOG.md#v041)
* [v0.4.0](CHANGELOG.md#v040)
* [v0.3.1](CHANGELOG.md#v031)
* [v0.3.0](CHANGELOG.md#v030)
* [v0.2.0](CHANGELOG.md#v020)
