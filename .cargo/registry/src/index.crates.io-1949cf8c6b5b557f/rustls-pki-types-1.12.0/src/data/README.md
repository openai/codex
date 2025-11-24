These files contain the binary DER encoding of the *values* of some
ASN.1 [`AlgorithmIdentifier`]s, without the outer `SEQUENCE` tag or the outer
length component.

These files were encoded with the help of [der-ascii]. They can be decoded
using:

```sh
go install github.com/google/der-ascii/cmd/der2ascii@latest
der2ascii -i <filename> -o <filename>.ascii
```

New or modified der-ascii files can be encoded using:

```sh
go install github.com/google/der-ascii/cmd/ascii2der@latest
ascii2der i <filename>.ascii -o <filename>
```

[`AlgorithmIdentifier`]: https://tools.ietf.org/html/rfc5280#section-4.1.1.2]
[der-ascii]: https://github.com/google/der-ascii
