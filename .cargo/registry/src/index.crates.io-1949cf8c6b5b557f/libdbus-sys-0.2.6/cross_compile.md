Cross compiling dbus
====================

There are (at least) three different approaches to cross compiling dbus. Choose which one fits you best:

 * Using rust-embedded/cross, see below
 * There is an alternative guide at [issue 184](https://github.com/diwic/dbus-rs/issues/184#issuecomment-520228758), which also contains a [powershell script](https://github.com/diwic/dbus-rs/issues/184#issuecomment-520791888) that set things up for you.
 * Setting things up manually, see below

The examples below all assume you're trying to compile for Raspberry Pi 2 or 3 running Raspbian. Adjust target triples accordingly if your target is something else.


Cross compiling using rust-embedded/cross
=========================================

The `vendored` feature is the current recommended way to cross compile dbus-rs:

```
dbus = {version = "0.9.7", features = ["vendored"]}
```

Then simply build your project with cross:

```
cross build --target arm-unknown-linux-musleabihf
```

#### Legacy Instructions:

Thanks to [jobale](https://github.com/jobale) for providing these instructions
(taken from [issue 292](https://github.com/diwic/dbus-rs/issues/292)).

Tested on Ubuntu 20.04 | rustc 1.47.0

- Install [rust-embedded/cross](https://github.com/rust-embedded/cross)
- In your project directory, create a **Cross.toml** file: `touch Cross.toml`
Add this code in it:

```
[target.armv7-unknown-linux-gnueabihf]
image = "rustcross:dbus-armhf"

[build.env]
passthrough = [
	"RUSTFLAGS",
]
```


- In your project directory create a **Dockerfile**: `touch Dockerfile`
Put this code in it:

```
# Base image for rapsberrypi 3 target
FROM rustembedded/cross:armv7-unknown-linux-gnueabihf

# Install libdbus libraries and pkg-config
RUN dpkg --add-architecture armhf && \
	    apt-get update && \
	    apt-get install --assume-yes libdbus-1-dev libdbus-1-dev:armhf pkg-config
```

For whatever reason, _in the docker image_, armhf libraries are installed in at least 2 locations. That's the reason of all my troubles:
- /usr/arm-linux-gnueabihf/lib/
- /usr/lib/arm-linux-gnueabihf/

- Cross needs to know those locations. We pass them to the compiler through a command flag. For convenience I put it in a bash script:

```
#!/bin/bash
set -o errexit
set -o nounset
set -o pipefail
set -o xtrace

readonly TARGET_ARCH=armv7-unknown-linux-gnueabihf
readonly LINK_FLAGS='-L /usr/arm-linux-gnueabihf/lib/ -L /usr/lib/arm-linux-gnueabihf/'

RUSTFLAGS=${LINK_FLAGS} cross build --release --target=${TARGET_ARCH}

```


Some more explanations for newcomers like me:

- Cross command act as cargo command (e.g: `cross build` is the same as `cargo build` but for cross-compiling).
- In Cross.toml,  `passthrough = [ "RUSTFLAGS",]` is what enable us to pass flags/parameters to the compiler in the docker image.



Setting up cross compiling manually
===================================

A cross linker
--------------

Apparently, `rustc` in itself can generate code for many archs but not assemble the generated code into the final executable. Hence you need a cross linker.

**Install it** - here follow whatever guide you have for the target arch. Distributions may also ship with cross toolchains. Example for Ubuntu 18.04:

`sudo apt install gcc-8-multilib-arm-linux-gnueabihf`

**Tell rustc where to find it** - in [.cargo/config](https://doc.rust-lang.org/cargo/reference/config.html) add the following:

```
[target.armv7-unknown-linux-gnueabihf]
linker = "arm-linux-gnueabihf-gcc-8"
```

Target rust std
---------------

This one's easy, just run rustup:

`rustup target add armv7-unknown-linux-gnueabihf`


Target dbus libraries
---------------------

**Installing the libraries**

Now to the more challenging part. Since we link to a C library `libdbus-1.so`, we also need the target version of that library. However, `libdbus-1.so` in itself links to a systemd library (at least it does so here) which in turn links to other libraries etc, so we need target versions of those libraries too.

Getting an entire rootfs/image is probably the easiest option. The rootfs needs to have `libdbus-1-dev` installed. I e:

 * Boot your target (i e, a raspberry), install `libdbus-1-dev` on it, turn it off and put the SD card into your computer's SD card reader. Mount the partition.
 * If the above is not an option, you could download an image, mount it at the right offset, like this (ugly hack!) `sudo mount -o loop,offset=50331648 2019-04-08-raspbian-stretch-lite.img /tmp/mnt` and then, to manually make the symlink that `libdbus-1-dev` does for you: `cd /tmp/mnt/usr/lib/arm-linux-gnueabihf && ln -s ../../../lib/arm-linux-gnueabihf/libdbus-1.so.3 libdbus-1.so`.
 * Or you can use the alternative guide's approach to download, extract and post-process the relevant `.deb` files manually. This might be a preferrable option if an entire image/rootfs would be too large.

**Finding the libraries**

When not cross compiling, finding the right library is done by a `build.rs` script which calls `pkg-config`. This will not work when cross compiling because it will point to the `libdbus-1.so` on the host, not the `libdbus-1.so` of the target.
Maybe it is possible to teach `pkg-config` how to return the target library instead, but I have not tried this approach. Instead we can override build script altogether and provide the same info manually. This is possible because `libdbus-sys` has a `links = dbus` line.

For the example below we assume that we have mounted a Raspbian rootfs on `/tmp/mnt`, and that the cross linker came with some basic libraries (libc, libpthread etc) that are installed on `/usr/arm-linux-gnueabihf/lib`. Unfortunately, we can't use the basic libraries that are present on the image, because they might contain absolute paths.

And so we add the following to [.cargo/config](https://doc.rust-lang.org/cargo/reference/config.html):

```
[target.armv7-unknown-linux-gnueabihf.dbus]
rustc-link-search = ["/usr/arm-linux-gnueabihf/lib", "/tmp/mnt/usr/lib/arm-linux-gnueabihf"]
rustc-link-lib = ["dbus-1"]
```


Finally
-------

If we are all set up, you should be able to successfully compile with:

`cargo build --target=armv7-unknown-linux-gnueabihf`


Docker compose
==============

Dockerfile
```docker
FROM rust:1.64.0-slim-bullseye 
 
RUN apt update && apt upgrade -y 
RUN apt install -y g++-arm-linux-gnueabihf libc6-dev-armhf-cross

RUN rustup target add armv7-unknown-linux-gnueabihf 
RUN rustup toolchain install stable-armv7-unknown-linux-gnueabihf 
 
RUN dpkg --add-architecture armhf 
RUN apt update
RUN apt install --assume-yes libdbus-1-dev libdbus-1-dev:armhf pkg-config

WORKDIR /app 
 
ENV CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc 
ENV CC_armv7_unknown_Linux_gnueabihf=arm-linux-gnueabihf-gcc 
ENV CXX_armv7_unknown_linux_gnueabihf=arm-linux-gnueabihf-g++
ENV CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=/usr/bin/arm-linux-gnueabihf-gcc
ENV PKG_CONFIG_ALLOW_CROSS="true"
ENV PKG_CONFIG_PATH="/usr/lib/arm-linux-gnueabihf/pkgconfig"
ENV RUSTFLAGS="-L /usr/arm-linux-gnueabihf/lib/ -L /usr/lib/arm-linux-gnueabihf/" 
```

docker-compose.yml
```yaml
version: "3"

services:
  compilation: 
    build: .
    volumes:
      - ./:/app
    command: "cargo build --release --target=armv7-unknown-linux-gnueabihf"
```

You just have to add these two files to your project root and run `docker-compose up`.
