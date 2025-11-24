# `supports-color` Release Changelog

<a name="3.0.2"></a>
## 3.0.2 (2024-11-26)

### Bug Fixes

* **unsafe:** Remove `unsafe` code in favor of `OnceLock` (#20) ([4b299e2a](https://github.com/zkat/supports-color/commit/4b299e2ab5417dc47cddd07b7527b86286cf21a5))

<a name="3.0.1"></a>
## 3.0.1 (2024-09-03)

### Bug Fixes

* **ansi:** improve detection of terminals over SSH (#19) ([370dc2b7](https://github.com/zkat/supports-color/commit/370dc2b754dd508e29fe1f586532e91a050f5ed5))

<a name="3.0.0"></a>
## 3.0.0 (2024-02-04)

### Features

* **deps:** Replace `is-terminal` with `std::io::IsTerminal` (#11) ([6fb6e359](https://github.com/zkat/supports-color/commit/6fb6e35961055a701264d879744f615c25b7629d))
    * **BREAKING CHANGE**: This bumps the MSRV to 1.70.0 due to the new `std::io::IsTerminal` API.

<a name="2.1.0"></a>
## 2.1.0 (2023-09-20)

### Features

* **ignore_is_terminal:** Allow the is_terminal check to be overridden by environment variable. (#13) ([ff0a31a6](https://github.com/zkat/supports-color/commit/ff0a31a672da89a48ad1978220184b91218fde32))
* **truecolor:** improve check for truecolor (#14) ([736c044a](https://github.com/zkat/supports-color/commit/736c044a9aa36e259fef25cd790c466f1bf7b011))

<a name="2.0.0"></a>
## 2.0.0 (2022-12-15)

### Bug Fixes

* **deps:** Replace atty with is_terminal (#10) ([edf565e5](https://github.com/zkat/supports-color/commit/edf565e553a2ad8b429a0b54ecec4128b6430e2b))
    * **BREAKING CHANGE**: Exported stream types are no longer atty's getting re-exported.

<a name="1.3.1"></a>
## 1.3.1 (2022-11-05)

### Bug Fixes

* **docs:** Update README example for accuracy (#7) ([88a56df7](https://github.com/zkat/supports-color/commit/88a56df7d3143cd71b1f5ad88b0f65ff6ddce8eb))
* **alloc:** stop allocating unnecessary Strings (#8) ([606262f7](https://github.com/zkat/supports-color/commit/606262f7c1fd117610b582fa28ae0acf60341164))

<a name="1.3.0"></a>
## 1.3.0 (2021-10-03)

### Bug Fixes

* **clicolor:** Ignore unit tests on GH action and change order of CLICOLOR check (#5) ([370e8188](https://github.com/zkat/supports-color/commit/370e81885bf683287cdb2f639b59b86425d90e9c))

### Features

* **msrv:** Reduce MSRV from 1.54 to 1.42 (#6) ([a33337a6](https://github.com/zkat/supports-color/commit/a33337a653d3bfe71007947cd3ee57a787dcce64))

<a name="1.2.0"></a>
## 1.2.0 (2021-10-01)

### Features

* **clicolor:** Add CLICOLOR support (#4) ([71b0ff30](https://github.com/zkat/supports-color/commit/71b0ff30be9a9aa78d2d0197957d815fc5d1a357))

<a name="1.1.1"></a>
## 1.1.1 (2021-09-22)

### Bug Fixes

* **deps:** bump is_ci because of missing api ([31d57374](https://github.com/zkat/supports-color/commit/31d5737420ae2a587e63f4ce03ad3099dad25289))

<a name="1.1.0"></a>
## 1.1.0 (2021-09-22)

### Features

* **api:** Add cached version of supports_color::on (#1) ([2cf2b76f](https://github.com/zkat/supports-color/commit/2cf2b76f585d591acda45c28bffeeba28d030bfd))

<a name="1.0.4"></a>
## 1.0.4 (2021-09-22)

### Bug Fixes

* **deps:** remove dependencies on regex and lazy-static too ([366f5b92](https://github.com/zkat/supports-color/commit/366f5b92c8c806572f74adc91e8716e434a95efb))

<a name="1.0.3"></a>
## 1.0.3 (2021-09-22)

### Bug Fixes

* **deps:** switch to much more lightweight is_ci ([d481c017](https://github.com/zkat/supports-color/commit/d481c01754ebaefa7bcaf154b8a7c7d8d25bebb5))

<a name="1.0.2"></a>
## 1.0.2 (2021-09-11)

### Bug Fixes

* **detection:** messed up logic on windows and forgot no_color. oops ([fbecf72f](https://github.com/zkat/supports-color/commit/fbecf72f6331ddc08de625861d9bece0370b07c3))

<a name="1.0.1"></a>
## 1.0.1 (2021-09-11)

### Bug Fixes

* **api:** forgot to export atty::Stream, and example was totally wrong ([a9eef66f](https://github.com/zkat/supports-color/commit/a9eef66f5fa6b75b14bbb4d860f24dba9dcf5724))

<a name="1.0.0"></a>
## 1.0.0 (2021-09-11)

### Features

* **api:** initial commit ([fbb068d0](https://github.com/zkat/supports-color/commit/fbb068d070687eac0ecbef23015c07cd025ce161))

