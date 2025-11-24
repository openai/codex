# @generated
load("//go/private:go_toolchain.bzl",
    "go_toolchain",
)
load("//go/private:toolchain.bzl",
    "go_download_sdk",
    "go_host_sdk",
)
load("//go/platform:list.bzl",
    "GOARCH",
    "GOOS",
    "GOOS_GOARCH",
)

DEFAULT_VERSION = "1.9.2"

SDK_REPOSITORIES = {
    "1.9.2": {
        "darwin_amd64":      ("go1.9.2.darwin-amd64.tar.gz", "73fd5840d55f5566d8db6c0ffdd187577e8ebe650c783f68bd27cbf95bde6743"),
        "linux_386":         ("go1.9.2.linux-386.tar.gz", "574b2c4b1a248e58ef7d1f825beda15429610a2316d9cbd3096d8d3fa8c0bc1a"),
        "linux_amd64":       ("go1.9.2.linux-amd64.tar.gz", "de874549d9a8d8d8062be05808509c09a88a248e77ec14eb77453530829ac02b"),
        "linux_armv6l":      ("go1.9.2.linux-armv6l.tar.gz", "8a6758c8d390e28ef2bcea511f62dcb43056f38c1addc06a8bc996741987e7bb"),
        "windows_386":       ("go1.9.2.windows-386.zip", "35d3be5d7b97c6d11ffb76c1b19e20a824e427805ee918e82c08a2e5793eda20"),
        "windows_amd64":     ("go1.9.2.windows-amd64.zip", "682ec3626a9c45b657c2456e35cadad119057408d37f334c6c24d88389c2164c"),
        "freebsd_386":       ("go1.9.2.freebsd-386.tar.gz", "809dcb0a8457c8d0abf954f20311a1ee353486d0ae3f921e9478189721d37677"),
        "freebsd_amd64":     ("go1.9.2.freebsd-amd64.tar.gz", "8be985c3e251c8e007fa6ecd0189bc53e65cc519f4464ddf19fa11f7ed251134"),
        "linux_arm64":       ("go1.9.2.linux-arm64.tar.gz", "0016ac65ad8340c84f51bc11dbb24ee8265b0a4597dbfdf8d91776fc187456fa"),
        "linux_ppc64le":     ("go1.9.2.linux-ppc64le.tar.gz", "adb440b2b6ae9e448c253a20836d8e8aa4236f731d87717d9c7b241998dc7f9d"),
        "linux_s390x":       ("go1.9.2.linux-s390x.tar.gz", "a7137b4fbdec126823a12a4b696eeee2f04ec616e9fb8a54654c51d5884c1345"),
    },
    "1.9.1": {
        "darwin_amd64":      ("go1.9.1.darwin-amd64.tar.gz", "59bc6deee2969dddc4490b684b15f63058177f5c7e27134c060288b7d76faab0"),
        "linux_386":         ("go1.9.1.linux-386.tar.gz", "2cea1ce9325cb40839601b566bc02b11c92b2942c21110b1b254c7e72e5581e7"),
        "linux_amd64":       ("go1.9.1.linux-amd64.tar.gz", "07d81c6b6b4c2dcf1b5ef7c27aaebd3691cdb40548500941f92b221147c5d9c7"),
        "linux_armv6l":      ("go1.9.1.linux-armv6l.tar.gz", "65a0495a50c7c240a6487b1170939586332f6c8f3526abdbb9140935b3cff14c"),
        "windows_386":       ("go1.9.1.windows-386.zip", "ea9c79c9e6214c9a78a107ef5a7bff775a281bffe8c2d50afa66d2d33998078a"),
        "windows_amd64":     ("go1.9.1.windows-amd64.zip", "8dc72a3881388e4e560c2e45f6be59860b623ad418e7da94e80fee012221cc81"),
        "freebsd_386":       ("go1.9.1.freebsd-386.tar.gz", "0da7ad96606a8ceea85652eb20816077769d51de9219d85b9b224a3390070c50"),
        "freebsd_amd64":     ("go1.9.1.freebsd-amd64.tar.gz", "c4eeacbb94821c5f252897a4d49c78293eaa97b29652d789dce9e79bc6aa6163"),
        "linux_arm64":       ("go1.9.1.linux-arm64.tar.gz", "d31ecae36efea5197af271ccce86ccc2baf10d2e04f20d0fb75556ecf0614dad"),
        "linux_ppc64le":     ("go1.9.1.linux-ppc64le.tar.gz", "de57b6439ce9d4dd8b528599317a35fa1e09d6aa93b0a80e3945018658d963b8"),
        "linux_s390x":       ("go1.9.1.linux-s390x.tar.gz", "9adf03574549db82a72e0d721ef2178ec5e51d1ce4f309b271a2bca4dcf206f6"),
    },
    "1.9": {
        "darwin_amd64":      ("go1.9.darwin-amd64.tar.gz", "c2df361ec6c26fcf20d5569496182cb20728caa4d351bc430b2f0f1212cca3e0"),
        "linux_386":         ("go1.9.linux-386.tar.gz", "7cccff99dacf59162cd67f5b11070d667691397fd421b0a9ad287da019debc4f"),
        "linux_amd64":       ("go1.9.linux-amd64.tar.gz", "d70eadefce8e160638a9a6db97f7192d8463069ab33138893ad3bf31b0650a79"),
        "linux_armv6l":      ("go1.9.linux-armv6l.tar.gz", "f52ca5933f7a8de2daf7a3172b0406353622c6a39e67dd08bbbeb84c6496f487"),
        "windows_386":       ("go1.9.windows-386.zip", "ecfe6f5be56acedc56cd9ff735f239a12a7c94f40b0ea9753bbfd17396f5e4b9"),
        "windows_amd64":     ("go1.9.windows-amd64.zip", "874b144b994643cff1d3f5875369d65c01c216bb23b8edddf608facc43966c8b"),
        "freebsd_386":       ("go1.9.freebsd-386.tar.gz", "9e415e340eaea526170b0fd59aa55939ff4f76c126193002971e8c6799e2ed3a"),
        "freebsd_amd64":     ("go1.9.freebsd-amd64.tar.gz", "ba54efb2223fb4145604dcaf8605d519467f418ab02c081d3cd0632b6b43b6e7"),
        "linux_ppc64le":     ("go1.9.linux-ppc64le.tar.gz", "10b66dae326b32a56d4c295747df564616ec46ed0079553e88e39d4f1b2ae985"),
        "linux_arm64":       ("go1.9.linux-arm64.tar.gz", "0958dcf454f7f26d7acc1a4ddc34220d499df845bc2051c14ff8efdf1e3c29a6"),
        "linux_s390x":       ("go1.9.linux-s390x.tar.gz", "e06231e4918528e2eba1d3cff9bc4310b777971e5d8985f9772c6018694a3af8"),
    },
    "1.8.5": {
        "darwin_amd64":      ("go1.8.5.darwin-amd64.tar.gz", "af5bd0c8e669a61f4b38fcce03bbf02f1ce672724a95c2ad61e89c6785f5c51e"),
        "linux_386":         ("go1.8.5.linux-386.tar.gz", "cf959b60b89acb588843ff985ecb47a7f6c37da6e4987739ab4aafad7211464f"),
        "linux_amd64":       ("go1.8.5.linux-amd64.tar.gz", "4f8aeea2033a2d731f2f75c4d0a4995b357b22af56ed69b3015f4291fca4d42d"),
        "linux_armv6l":      ("go1.8.5.linux-armv6l.tar.gz", "f5c58e7fd6cdfcc40b94c6655cf159b25836dffe13431f683b51705b8a67d608"),
        "windows_386":       ("go1.8.5.windows-386.zip", "c14d800bb79bf38a945f83cf37005609b719466c0051d20a5fc59d6efdd6fc66"),
        "windows_amd64":     ("go1.8.5.windows-amd64.zip", "137827cabff27cc36cbe13018f629a6418c2a6af85adde1b1bfb8d000c9fc1ae"),
        "freebsd_386":       ("go1.8.5.freebsd-386.tar.gz", "b7e246c9ec1b68e481abe6190caf79cc7179b9308c30076081a9dc90b3a12f99"),
        "freebsd_amd64":     ("go1.8.5.freebsd-amd64.tar.gz", "8a025284c1911aba8d133e9fcadd6a6dcf5dc78b0d8139be88747cea09773407"),
        "linux_ppc64le":     ("go1.8.5.linux-ppc64le.tar.gz", "1ee0874ce8c8625e14b4457a4861777be78f30067d914bcb264f7e0331d087de"),
        "linux_s390x":       ("go1.8.5.linux-s390x.tar.gz", "e978a56842297dc8924555540314ff09128e9a62da9881c3a26771ddd5d7ebc2"),
    },
    "1.8.4": {
        "darwin_amd64":      ("go1.8.4.darwin-amd64.tar.gz", "cf803053aec24425d7be986af6dff0051bb48527bcdfa5b9ffeb4d40701ab54e"),
        "linux_386":         ("go1.8.4.linux-386.tar.gz", "00354388d5f7d21b69c62361e73250d2633124e8599386f704f6dd676a2f82ac"),
        "linux_amd64":       ("go1.8.4.linux-amd64.tar.gz", "0ef737a0aff9742af0f63ac13c97ce36f0bbc8b67385169e41e395f34170944f"),
        "linux_armv6l":      ("go1.8.4.linux-armv6l.tar.gz", "76329898bb9f2be0f86b07f05a6336818cb12f3a416ab3061aa0d5f2ea5c6ff0"),
        "windows_386":       ("go1.8.4.windows-386.zip", "c0f949174332e5b9d4f025c84338bbec1c94b436f249c20aade04a024537f0be"),
        "windows_amd64":     ("go1.8.4.windows-amd64.zip", "2ddfea037fd5e2eeb0cb854c095f6e44aaec27e8bbf76dca9a11a88e3a49bbf7"),
        "freebsd_386":       ("go1.8.4.freebsd-386.tar.gz", "4764920bc94cc9723e7a9a65ae7764922e0ab6148e1cf206bbf37062997fdf4c"),
        "freebsd_amd64":     ("go1.8.4.freebsd-amd64.tar.gz", "21dd9899b91f4aaeeb85c7bb7db6cd4b44be089b2a7397ea8f9f2e3397a0b5c6"),
        "linux_ppc64le":     ("go1.8.4.linux-ppc64le.tar.gz", "0f043568d65fd8121af6b35a39f4f20d292a03372b6531e80b743ee0689eb717"),
        "linux_s390x":       ("go1.8.4.linux-s390x.tar.gz", "aa998b7ac8882c549f7017d2e9722a3102cb9e6b92010baf5153a6dcf98205b1"),
    },
    "1.8.3": {
        "darwin_amd64":      ("go1.8.3.darwin-amd64.tar.gz", "f20b92bc7d4ab22aa18270087c478a74463bd64a893a94264434a38a4b167c05"),
        "linux_386":         ("go1.8.3.linux-386.tar.gz", "ff4895eb68fb1daaec41c540602e8bb4c1e8bb2f0e7017367171913fc9995ed2"),
        "linux_amd64":       ("go1.8.3.linux-amd64.tar.gz", "1862f4c3d3907e59b04a757cfda0ea7aa9ef39274af99a784f5be843c80c6772"),
        "linux_armv6l":      ("go1.8.3.linux-armv6l.tar.gz", "3c30a3e24736ca776fc6314e5092fb8584bd3a4a2c2fa7307ae779ba2735e668"),
        "windows_386":       ("go1.8.3.windows-386.zip", "9e2bfcb8110a3c56f23b91f859963269bc29fd114190fecfd0a539395272a1c7"),
        "windows_amd64":     ("go1.8.3.windows-amd64.zip", "de026caef4c5b4a74f359737dcb2d14c67ca45c45093755d3b0d2e0ee3aafd96"),
        "freebsd_386":       ("go1.8.3.freebsd-386.tar.gz", "d301cc7c2b8b0ccb384ac564531beee8220727fd27ca190b92031a2e3e230224"),
        "freebsd_amd64":     ("go1.8.3.freebsd-amd64.tar.gz", "1bf5f076d48609012fe01b95e2a58e71e56719a04d576fe3484a216ad4b9c495"),
        "linux_ppc64le":     ("go1.8.3.linux-ppc64le.tar.gz", "e5fb00adfc7291e657f1f3d31c09e74890b5328e6f991a3f395ca72a8c4dc0b3"),
        "linux_s390x":       ("go1.8.3.linux-s390x.tar.gz", "e2ec3e7c293701b57ca1f32b37977ac9968f57b3df034f2cc2d531e80671e6c8"),
    },
    "1.8.2": {
        "linux_amd64":       ("go1.8.2.linux-amd64.tar.gz", "5477d6c9a4f96fa120847fafa88319d7b56b5d5068e41c3587eebe248b939be7"),
        "darwin_amd64":      ("go1.8.2.darwin-amd64.tar.gz", "3f783c33686e6d74f6c811725eb3775c6cf80b9761fa6d4cebc06d6d291be137"),
    },
    "1.8.1": {
        "linux_amd64":       ("go1.8.1.linux-amd64.tar.gz", "a579ab19d5237e263254f1eac5352efcf1d70b9dacadb6d6bb12b0911ede8994"),
        "darwin_amd64":      ("go1.8.1.darwin-amd64.tar.gz", "25b026fe2f4de7c80b227f69588b06b93787f5b5f134fbf2d652926c08c04bcd"),
    },
    "1.8": {
        "linux_amd64":       ("go1.8.linux-amd64.tar.gz", "3ab94104ee3923e228a2cb2116e5e462ad3ebaeea06ff04463479d7f12d27ca"),
        "darwin_amd64":      ("go1.8.darwin-amd64.tar.gz", "fdc9f98b76a28655a8770a1fc8197acd8ef746dd4d8a60589ce19604ba2a120"),
    },
}

def _generate_toolchains():
  # Use all the above information to generate all the possible toolchains we might support
  toolchains = []
  for host_goos, host_goarch in GOOS_GOARCH:
    host = "{}_{}".format(host_goos, host_goarch)
    for target_goos, target_goarch in GOOS_GOARCH:
      target = "{}_{}".format(target_goos, target_goarch)
      toolchain_name = "go_{}".format(host)
      if host != target:
        toolchain_name += "_cross_" + target
      link_flags = []
      cgo_link_flags = []
      if "darwin" in host:
        # workaround for a bug in ld(1) on Mac OS X.
        # http://lists.apple.com/archives/Darwin-dev/2006/Sep/msg00084.html
        # TODO(yugui) Remove this workaround once rules_go stops supporting XCode 7.2
        # or earlier.
        link_flags.append("-s")
        cgo_link_flags.extend(["-shared", "-Wl,-all_load"])
      if "linux" in host:
        cgo_link_flags.append("-Wl,-whole-archive")
      # Add the primary toolchain
      toolchains.append(dict(
          name = toolchain_name,
          host = host,
          target = target,
          link_flags = link_flags,
          cgo_link_flags = cgo_link_flags,
      ))
  return toolchains

_toolchains = _generate_toolchains()
_label_prefix = "@io_bazel_rules_go//go/toolchain:"

def go_register_toolchains(go_version=DEFAULT_VERSION):
  """See /go/toolchains.rst#go-register-toolchains for full documentation."""
  if "go_sdk" not in native.existing_rules():
    if go_version in SDK_REPOSITORIES:
      go_download_sdk(
          name = "go_sdk",
          sdks = SDK_REPOSITORIES[go_version],
      )
    elif go_version == "host":
      go_host_sdk(
          name = "go_sdk"
      )
    else:
      fail("Unknown go version {}".format(go_version))

  # Use the final dictionaries to register all the toolchains
  for toolchain in _toolchains:
    name = _label_prefix + toolchain["name"]
    native.register_toolchains(name)
    if toolchain["host"] == toolchain["target"]:
      name = name + "-bootstrap"
      native.register_toolchains(name)

def declare_constraints():
  for goos, constraint in GOOS.items():
    if constraint:
      native.alias(
          name = goos,
          actual = constraint,
      )
    else:
      native.constraint_value(
          name = goos,
          constraint_setting = "@bazel_tools//platforms:os",
      )
  for goarch, constraint in GOARCH.items():
    if constraint:
      native.alias(
          name = goarch,
          actual = constraint,
      )
    else:
      native.constraint_value(
          name = goarch,
          constraint_setting = "@bazel_tools//platforms:cpu",
      )
  for goos, goarch in GOOS_GOARCH:
    native.platform(
        name = goos + "_" + goarch,
        constraint_values = [
            ":" + goos,
            ":" + goarch,
        ],
    )

def declare_toolchains():
  # Use the final dictionaries to create all the toolchains
  for toolchain in _toolchains:
    go_toolchain(
        # Required fields
        name = toolchain["name"],
        host = toolchain["host"],
        target = toolchain["target"],
        # Optional fields
        link_flags = toolchain["link_flags"],
        cgo_link_flags = toolchain["cgo_link_flags"],
    )
