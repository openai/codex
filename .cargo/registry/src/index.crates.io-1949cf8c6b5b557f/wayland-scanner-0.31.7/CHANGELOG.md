# CHANGELOG: wayland-scanner

## Unreleased

- Update `quick-xml` to 0.37

## 0.31.5 -- 2024-09-04

- Update `quick-xml` to 0.36
- Allow `<!DOCTYPE>` in XML input

## 0.31.3 -- 2024-07-03

- Update `quick-xml` to 0.34

## 0.31.2 -- 2024-05-30

- Use wrapper type implementing `Sync` instead of `static mut`s.
- Add headerless xml file parsing possibility for `parse` function.

## 0.31.1 -- 2024-01-29

- Include an `std::convert::Infallible` in hidden `__phantom_lifetime` enum variants,
  so they're explicitly unconstructable.

## 0.31.0 -- 2023-09-02

#### Breaking changes

- Bump bitflags to 2.0
- Remove `io-lifetimes` from the generated code following wayland-backend 0.3 dropping it.
- Generate `BorrowedFd<'_>` arguments instead of `RawFd`

# 0.30.1 -- 2023-06-17

- Generated protocol object types now implement `Borrow<ObjectId>` and `Hash`

## 0.30.0 -- 2022-12-27

## 0.30.0-beta.10

#### Additions

- An `opcode()` method is added to message enums to retrive the opcode associated with a message variant.

## 0.30.0-beta.9

- Migrate from xml-rs to quick-xml

## 0.30.0-beta.6

- Generated enums now derive `Ord` and `Hash`.
- The scanner now generates constants for the opcode values of the protocol messages.

## 0.30.0-alpha1

Full rework of the crate together of the reworks of `wayland-client` and `wayland-server`.

The code generation is now achieved using procedural macros rather than build scripts.
