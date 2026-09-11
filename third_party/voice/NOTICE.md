# Native voice libraries in Codex releases

Codex release packages with voice include dynamically linked GStreamer and GLib
libraries and selected plugins, plus their native library dependencies. These
components have their own copyrights and licenses. Their notices accompany
this file in `licenses/`: `LGPL-2.1.txt` for GStreamer and GLib,
`proxy-libintl.txt` for libintl, `libffi.txt`, `PCRE2.md` and `sljit.txt`
for PCRE2, `Opus.txt`, and `zlib.txt`. GVDB is included with GLib under LGPL.

The exact upstream versions, source archive URLs and SHA-256 digests are in
`sources.json` alongside this notice. The corresponding build, runtime
projection and package scripts are in the public Codex source tree under
`third_party/voice/`. The source commit for this package is recorded in
`manifest.json`. The native libraries remain separate dynamic libraries in
platform-specific runtime directories. Replacements must be compatible with
the package and, on macOS, have valid code signatures. Build tools listed in
`sources.json` are build inputs, not bundled runtime libraries.

## Microsoft Visual C++ runtime (Windows)

Windows packages also contain the unmodified Microsoft Visual C++ runtime DLL
`bin/vcruntime140.dll`, relative to this notice in release packages,
copyright Microsoft Corporation, under Microsoft's applicable software license
terms, separately from the open-source audio libraries. `windows-crt.json`
records its version, official download, hashes, and redistribution references.
The Apache and LGPL licenses for other components do not license this DLL.
Microsoft's runtime terms and separate developer redistribution terms apply
to the Microsoft component; the runtime terms alone do not grant redistribution.
Only retail redistributable files are included; Microsoft signatures are retained.
App-local runtime security updates must be delivered with Codex updates.

Microsoft provides the following license terms and redistribution information:

- [Visual C++ v14 Redistributable and Runtime license terms](https://visualstudio.microsoft.com/license-terms/vs2026-ga-visualcpp-v14-redist-runtime/)
- [Visual C++ Redistributable downloads and support information](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist?view=msvc-170)
- [Visual Studio 2026 redistribution information](https://learn.microsoft.com/en-us/visualstudio/releases/2026/redistribution)
