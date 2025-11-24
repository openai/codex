# Releases

## Version 2.10.1

* ipnet-2.10.0 Rust crate has executable permission on source files. #59

## Version 2.10.0

* Add new_assert implementations #57 
* Remove redundant curly braces so that the example program can compile #53 

## Version 2.9.0

*  Add ser_as_str feature to serialize using serialize_str() #52 

## Version 2.8.0

* Add no_std support on nightly #51 

## Version 2.7.2

* Inline constructors and field getters #48 
* Use Serializer::collect_str to serialize output of Display #39 

## Version 2.7.1

* Fix overflow in mask to prefix conversion #47

## Version 2.7.0

* Allow to invoke some functions as const fn #43 

## Version 2.6.0

* Add IP netmask to prefix conversion functions and new `with_netmask()` constructors.

## Version 2.5.0

* Manually implement JsonSchema for IpNet, Ipv4Net, Ipv6Net #41 because default derived JsonSchema does not correspond to Serde representation #40
* Migrate to edition 2018

## Version 2.4.0

* Add 'schemars' feature for deriving JSON schema #31 
* add convenience IpNet::new method #36 

## Version 2.3.1 (2020-06-15)

* Merge Fix Error::description() deprecation warning #28.

## Version 2.3.0 (2020-03-15)

* Merge @imp's `Default` implementation. See #18. `Ipv4Net` and `Ipv6Net` now default to 0.0.0.0/0 and ::/0 respectively. `IpNet` defaults to the 0/0 `Ipv4Net`.

* Add `#[allow(arithmetic_overflow)]` for `Ipv4AddrRange::count()` and `Ipv6AddrRange::count()`. Since 1.43.0-nightly it gives a build error but this panic behavior is desired. In future it may be replaced with explicit use of `panic!`. See #21.

## Version 2.2.0 (2020-02-02)

* Implement `From<IpAddr>`, `From<Ipv4Addr>`, and `From<Ipv6Addr>` for `IpNet`, `Ipv4Net`, and `Ipv6Net` respectively.

## Version 2.1.0 (2019-11-08)

* Implement `FusedIterator` for `IpAddrRange`, `Ipv4AddrRange`, `Ipv6AddrRange`, `IpSubnets`, `Ipv4Subnets`, and `Ipv6Subnets`.

* Implement `DoubleEndedIterator` for `IpAddrRange`, `Ipv4AddrRange`, `Ipv6AddrRange`.

* Implement custom `count()`, `last()`, `max()`, `min()`, `nth()`, and `size_hint()` for `IpAddrRange`, `Ipv4AddrRange`, `Ipv6AddrRange`.

## Version 2.0.1 (2019-10-12)

* Fix bug where IpAddrRange never ends when start and end are both 0 #11

* Fix warning about missing 'dyn'

## Version 2.0.0 (2018-08-21)

* The `Emu128` module has been removed. This provided an emulated 128-bit integer for supporting IPv6 addresses. As of Rust 1.26 the built-in 128-bit integers have been marked stable and this library has been updated to use these instead of `Emu128`.

* The `with-serde` feature name shim has been removed. The `serde` feature should now be used using the bare `serde` feature name per the Rust API Guidelines.

* The `Deref` on `Ipv4Net` and `Ipv6Net` has been removed. This dereferenced to the `Ipv4Addr` and `Ipv6Addr` contained in the type. To use these methods call them directly on the contained IP address of interest, which may be accessed using the `addr()` or `network()` methods.

* In prior versions it was necessary to use the `Contains` trait to access the `contains()` methods. These are now inherited in public methods on the `IpNet`, `Ipv4Net`, and `Ipv6Net` types so are always available.

* The implementations of `IpAdd<u32>` and `IpSub<u32>` for IpAddr have been removed.

* The implementations of `IpAdd<u32>` and `IpSub<u32>` for `Ipv6Addr` have been removed.

## Version 1.2.1 (2018-06-06)

* Fix to resolve an issue with the optional serde support, where compact binary formats were not properly supported. See issue #10.

## Version 1.2.0 (2018-04-17)

* The previous release (1.1.0) introduced serde support using the feature name `with-serde`, but the Rust API Guidelines recommend using `serde` as the name of the feature. This release changes the feature name from `with-serde` to `serde`, but it is backwards compatible for those that already started using the `with-serde` feature name. The 1.1.0 release was yanked on crates.io to discourage further use of this feature name. See pull request #7.

## Version 1.1.0 (2018-04-13)

* Adds serde support. See pull request #6.
