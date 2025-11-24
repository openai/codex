# Contributing

Thanks for your interest in contributing to rust-landlock!

## Testing vs kernel ABI

The Landlock functionality exposed differs between kernel versions.  In order
to test all possible variations, the rust-landlock tests will run different
subsets of tests based on the landlock support in the current kernel.

In order to fully test a change, it should be verified against a range of
kernel versions.  This is done by the github actions CI, but is currently
challenging to do locally.  For local development, running `cargo test` will
test against your currently running kernel version, which may result in some
tests being skipped.

The kernel to test against can be overridden using the LANDLOCK_CRATE_TEST_ABI
environmental variable.  For more details see the comment in
`compat.rs:current_kernel_abi()`.

For more information about Landlock ABIs see https://landlock.io/rust-landlock/landlock/enum.ABI.html
