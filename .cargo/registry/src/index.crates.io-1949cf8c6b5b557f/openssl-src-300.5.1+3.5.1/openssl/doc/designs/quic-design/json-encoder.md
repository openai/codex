JSON Encoder
============

Approach
--------

The JSON encoder exists to support qlog implementation. There is no intention to
implement a decoder at this time. The encoder is intended to support automation
using immediate calls without the use of an intermediate syntax tree
representation and is expected to be zero-allocation in most cases. This enables
highly efficient serialization when called from QUIC code without dynamic memory
allocation.

An example usage is as follows:

```c
int generate_json(BIO *b)
{
    int ret = 1;
    JSON_ENC z;

    if (!ossl_json_init(&z, b, 0))
        return 0;

    ossl_json_object_begin(&z);
    {
        ossl_json_key(&z, "key");
        ossl_json_str(&z, "value");

        ossl_json_key(&z, "key2");
        ossl_json_u64(&z, 42);

        ossl_json_key(&z, "key3");
        ossl_json_array_begin(&z);
        {
            ossl_json_null(&z);
            ossl_json_f64(&z, 42.0);
            ossl_json_str(&z, "string");
        }
        ossl_json_array_end(&z);
    }
    ossl_json_object_end(&z);

    if (ossl_json_get_error_flag(&z))
        ret = 0;

    ossl_json_cleanup(&z);
    return ret;
}
```

The zero-allocation, immediate-output design means that most API calls
correspond directly to immediately generated output; however there is some
minimal state tracking. The API guarantees that it will never generate invalid
JSON, with two exceptions:

- it is the caller's responsibility to avoid generating duplicate keys;
- it is the caller's responsibility to provide valid UTF-8 strings.

Since the JSON encoder is for internal use only, its structure is defined in
headers and can be incorporated into other objects without a heap allocation.
The JSON encoder maintains an internal write buffer and a small state tracking
stack (1 bit per level of depth in a JSON hierarchy).

JSON-SEQ
--------

The encoder supports JSON-SEQ (RFC 7464), as this is an optimal format for
outputting qlog for our purposes.

Number Handling
---------------

It is an unfortunate reality that many JSON implementations are not able to
handle integers outside `[-2**53 + 1, 2**53 - 1]`. This leads to the I-JSON
specification, RFC 7493, which recommends that values outside these ranges are
encoded as strings.

An optional I-JSON mode is offered, in which case integers outside these ranges
are automatically serialized as strings instead.

Error Handling
--------------

Error handling is deferred to improve ergonomics. If any call to a JSON encoder
fails, all future calls also fail and the caller is expected to ascertain that
the encoding process failed by calling `ossl_json_get_error_flag`.

API
---

The API is documented in `include/internal/json_enc.h`.
