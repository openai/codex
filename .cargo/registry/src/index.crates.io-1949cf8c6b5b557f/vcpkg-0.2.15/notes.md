# Notes

## Making a release

1. run vcpkg_cli and test it
1. run systest\test.cmd
1. check that everything is committed and work dir is clean
1. push to master on github
1. check that github actions ci passes
1. update changelog, commit and push
1. update version number in Cargo.toml for the crate to be released
1. commit
1. push
1. wait for the ci to work
1. create a tag for the right crate like `git tag vcpkg-rs-0.2.3`
1. cd to the crate dir and run `cargo publish`
1. git push origin --tags

## Possible future features

- hide or deprecate or note that the lib_name api is not as good as find_package

- make sure the find_package api is first in the docs and mention that it's the best option

- allow specifying a triple to use using an environment variable. this will allow setting up a custom "x64-rust-static" triple that dynamically links to msvcrt, allowing static builds with the default rust.

- add information about target triples and target triple selection being driven by RUSTFLAGS=-Ctarget-feature=+crt-static

- add a note that even rust debug builds are linked against the release version
  of built libraries

- there is a lib\no_auto_link folder that some packages generate that needs
  to be added to the link line. this will require finding an example of
  a library that uses that feature. (boost?)

- vcpkg_cli: make probe failure return a nonzero exit code so the build fails

- remove crate doc info about the libname -> package mapping. (why?)

- look into the possibility of using dotenv to allow setting VCPKG_ROOT

- could run vcpkg and parse it's output to determine what package versions are installed.

- could parse vcpkg's installed files list to guess at the names for libraries and dlls rather than requiring them to be specified.

- could parse vcpkg's installed packages list to determine what other packages we need to link against.

- vcpkg has common include and lib dirs so there is a chance that someone is going to end up picking up a vcpkg lib on their link line in preference to some other version at some point. I believe cmake handles this by using absolute paths for libs wherever possible. if everything below you in the dependency tree is looking in vcpkg then everything will agree.

- vcpkg has a per-package output dir that looks like it would be helpful, but at present it is undocumented and subject to change. (what I read mentioned the possibility of compressing the contents.)

- warn if you use something that looks like a vcpkg triplet in place of a rust triple

- allow specifying of the library to be installed like pkg-config does. (hard in general because there is no specific format for version numbers )

- allow stipulating that a specific feature be installed. at present if a feature is installed any extra libraries it requires will be linked as expected. how should this be? The vcpkg way is to specify it as harfbuzz[graphite2,icu] for example.

- report enabled/available features in the Library returned from find_package

- get information about installed packages by running the vcpkg executable

  - if using json to encode the information, this requires writing a json parser or adding a dependency on serde for anything that transitively depends on vcpkg, which is a lot of stuff, probably only a tiny percentage of which actually uses the vcpkg functionality. otherwise, could invent yet another easy-to-parse custom format.
  - vcpkg is now available on linux and macos also. a possible use is to build the whole bag of windows dependencies on a windows machine at the point that lld can cross link windows/msvc code.

- add a changelog for vcpkg_cli

- make the breaking change of dropping Rust 1.10 (actually 1.12) compatibility when updating to 0.3

- vcpkg_cli should say if there are other versions of the ports available that do not match what is being looked for

- add some commentary describing the vcpkg target triplets on windows - static vs dynamic crt and the fact that rust prefers something in between - dynamic crt, static libs on top of that.

- vcpkg now has a community supported triplet called x64-windows-static-md which is a match for the default Rust build type on windows - dynamic linking to the c runtime, and static linking to other libraries.

## Creating empty files from list files

```sh
touch `grep -h \.lib$ vcpkg/info/*x86* | grep -v debug `
touch `grep -h \.dll$ vcpkg/info/*x86* | grep -v debug `
```

