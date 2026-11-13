"""Expose the Ubuntu 24.04 bubblewrap executable as a Bazel test fixture."""

def _extract(ctx):
    ctx.download_and_extract(
        url = "https://archive.ubuntu.com/ubuntu/pool/main/b/bubblewrap/bubblewrap_0.9.0-1ubuntu0.3_amd64.deb",
        sha256 = "2461f1beee9cb04c8942739fe1a2b37e7b7c2a3d518f0779dc75f9245baa3094",
        type = "deb",
    )
    ctx.extract("data.tar.zst")
    ctx.file("BUILD.bazel", 'exports_files(["usr/bin/bwrap"])')
    return ctx.repo_metadata(reproducible = True)

managed_tcp_test_bwrap = repository_rule(implementation = _extract)
