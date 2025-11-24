THE SOFTWARE IS PROVIDED "AS IS" AND BRIAN SMITH AND THE AUTHORS DISCLAIM
ALL WARRANTIES WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES
OF MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL BRIAN SMITH OR THE AUTHORS
BE LIABLE FOR ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY
DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN
AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.


Most of the C and assembly language code in *ring* comes from BoringSSL. 
BoringSSL is a fork of OpenSSL. This quote from the BoringSSL README.md 
discouraging you from using it applies to this project:

> BoringSSL is a fork of OpenSSL that is designed to meet Google's needs.
>
> Although BoringSSL is an open source project, it is not intended for general
> use, as OpenSSL is. We don't recommend that third parties depend upon it.

This project was originally shared on GitHub in 2015 as an experiment. It was
put on crates.io shortly to help other people with their experiments. It is an
experiment.


Side Channels
-------------

See [SIDE-CHANNELS.md](SIDE-CHANNELS.md) for important information regarding
the limitations of the side channel mitigations in this project.


Toolchains & Targets
--------------------

Be especially weary about using toolchains (C compilers, etc.) or targets
that aren't supported by other projects, especially BoringSSL. The further you
are from using the same version of Clang that Chrome uses, the more weary you
should be.


Bug Reporting
-------------

For security vulnerabilities, see https://github.com/briansmith/ring/security/policy.

Please report bugs that aren't security vulnerabilities either as pull requests or as issues in
[the issue tracker](https://github.com/briansmith/ring/issues).



Release Notes
-------------
It is recommended that you review every commit in this project. Some
particularly noteworthy changes are noted in the [RELEASES.md](RELEASES.md). We could use some
help in making this better.
