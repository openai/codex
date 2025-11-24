Teddy is a SIMD accelerated multiple substring matching algorithm. The name
and the core ideas in the algorithm were learned from the [Hyperscan][1_u]
project. The implementation in this repository was mostly motivated for use in
accelerating regex searches by searching for small sets of required literals
extracted from the regex.


# Background

The key idea of Teddy is to do *packed* substring matching. In the literature,
packed substring matching is the idea of examining multiple bytes in a haystack
at a time to detect matches. Implementations of, for example, memchr (which
detects matches of a single byte) have been doing this for years. Only
recently, with the introduction of various SIMD instructions, has this been
extended to substring matching. The PCMPESTRI instruction (and its relatives),
for example, implements substring matching in hardware. It is, however, limited
to substrings of length 16 bytes or fewer, but this restriction is fine in a
regex engine, since we rarely care about the performance difference between
searching for a 16 byte literal and a 16 + N literal; 16 is already long
enough. The key downside of the PCMPESTRI instruction, on current (2016) CPUs
at least, is its latency and throughput. As a result, it is often faster to
do substring search with a Boyer-Moore (or Two-Way) variant and a well placed
memchr to quickly skip through the haystack.

There are fewer results from the literature on packed substring matching,
and even fewer for packed multiple substring matching. Ben-Kiki et al. [2]
describes use of PCMPESTRI for substring matching, but is mostly theoretical
and hand-waves performance. There is other theoretical work done by Bille [3]
as well.

The rest of the work in the field, as far as I'm aware, is by Faro and Kulekci
and is generally focused on multiple pattern search. Their first paper [4a]
introduces the concept of a fingerprint, which is computed for every block of
N bytes in every pattern. The haystack is then scanned N bytes at a time and
a fingerprint is computed in the same way it was computed for blocks in the
patterns. If the fingerprint corresponds to one that was found in a pattern,
then a verification step follows to confirm that one of the substrings with the
corresponding fingerprint actually matches at the current location. Various
implementation tricks are employed to make sure the fingerprint lookup is fast;
typically by truncating the fingerprint. (This may, of course, provoke more
steps in the verification process, so a balance must be struck.)

The main downside of [4a] is that the minimum substring length is 32 bytes,
presumably because of how the algorithm uses certain SIMD instructions. This
essentially makes it useless for general purpose regex matching, where a small
number of short patterns is far more likely.

Faro and Kulekci published another paper [4b] that is conceptually very similar
to [4a]. The key difference is that it uses the CRC32 instruction (introduced
as part of SSE 4.2) to compute fingerprint values. This also enables the
algorithm to work effectively on substrings as short as 7 bytes with 4 byte
windows. 7 bytes is unfortunately still too long. The window could be
technically shrunk to 2 bytes, thereby reducing minimum length to 3, but the
small window size ends up negating most performance benefits—and it's likely
the common case in a general purpose regex engine.

Faro and Kulekci also published [4c] that appears to be intended as a
replacement to using PCMPESTRI. In particular, it is specifically motivated by
the high throughput/latency time of PCMPESTRI and therefore chooses other SIMD
instructions that are faster. While this approach works for short substrings,
I personally couldn't see a way to generalize it to multiple substring search.

Faro and Kulekci have another paper [4d] that I haven't been able to read
because it is behind a paywall.


# Teddy

Finally, we get to Teddy. If the above literature review is complete, then it
appears that Teddy is a novel algorithm. More than that, in my experience, it
completely blows away the competition for short substrings, which is exactly
what we want in a general purpose regex engine. Again, the algorithm appears
to be developed by the authors of [Hyperscan][1_u]. Hyperscan was open sourced
late 2015, and no earlier history could be found. Therefore, tracking the exact
provenance of the algorithm with respect to the published literature seems
difficult.

At a high level, Teddy works somewhat similarly to the fingerprint algorithms
published by Faro and Kulekci, but Teddy does it in a way that scales a bit
better. Namely:

1. Teddy's core algorithm scans the haystack in 16 (for SSE, or 32 for AVX)
   byte chunks. 16 (or 32) is significant because it corresponds to the number
   of bytes in a SIMD vector.
2. Bitwise operations are performed on each chunk to discover if any region of
   it matches a set of precomputed fingerprints from the patterns. If there are
   matches, then a verification step is performed. In this implementation, our
   verification step is naive. This can be improved upon.

The details to make this work are quite clever. First, we must choose how to
pick our fingerprints. In Hyperscan's implementation, I *believe* they use the
last N bytes of each substring, where N must be at least the minimum length of
any substring in the set being searched. In this implementation, we use the
first N bytes of each substring. (The tradeoffs between these choices aren't
yet clear to me.) We then must figure out how to quickly test whether an
occurrence of any fingerprint from the set of patterns appears in a 16 byte
block from the haystack. To keep things simple, let's assume N = 1 and examine
some examples to motivate the approach. Here are our patterns:

```ignore
foo
bar
baz
```

The corresponding fingerprints, for N = 1, are `f`, `b` and `b`. Now let's set
our 16 byte block to:

```ignore
bat cat foo bump
xxxxxxxxxxxxxxxx
```

To cut to the chase, Teddy works by using bitsets. In particular, Teddy creates
a mask that allows us to quickly compute membership of a fingerprint in a 16
byte block that also tells which pattern the fingerprint corresponds to. In
this case, our fingerprint is a single byte, so an appropriate abstraction is
a map from a single byte to a list of patterns that contain that fingerprint:

```ignore
f |--> foo
b |--> bar, baz
```

Now, all we need to do is figure out how to represent this map in vector space
and use normal SIMD operations to perform a lookup. The first simplification
we can make is to represent our patterns as bit fields occupying a single
byte. This is important, because a single SIMD vector can store 16 bytes.

```ignore
f |--> 00000001
b |--> 00000010, 00000100
```

How do we perform lookup though? It turns out that SSSE3 introduced a very cool
instruction called PSHUFB. The instruction takes two SIMD vectors, `A` and `B`,
and returns a third vector `C`. All vectors are treated as 16 8-bit integers.
`C` is formed by `C[i] = A[B[i]]`. (This is a bit of a simplification, but true
for the purposes of this algorithm. For full details, see [Intel's Intrinsics
Guide][5_u].) This essentially lets us use the values in `B` to lookup values
in `A`.

If we could somehow cause `B` to contain our 16 byte block from the haystack,
and if `A` could contain our bitmasks, then we'd end up with something like
this for `A`:

```ignore
    0x00 0x01 ... 0x62      ... 0x66      ... 0xFF
A = 0    0        00000110      00000001      0
```

And if `B` contains our window from our haystack, we could use shuffle to take
the values from `B` and use them to look up our bitsets in `A`. But of course,
we can't do this because `A` in the above example contains 256 bytes, which
is much larger than the size of a SIMD vector.

Nybbles to the rescue! A nybble is 4 bits. Instead of one mask to hold all of
our bitsets, we can use two masks, where one mask corresponds to the lower four
bits of our fingerprint and the other mask corresponds to the upper four bits.
So our map now looks like:

```ignore
'f' & 0xF = 0x6 |--> 00000001
'f' >> 4  = 0x6 |--> 00000111
'b' & 0xF = 0x2 |--> 00000110
'b' >> 4  = 0x6 |--> 00000111
```

Notice that the bitsets for each nybble correspond to the union of all
fingerprints that contain that nybble. For example, both `f` and `b` have the
same upper 4 bits but differ on the lower 4 bits. Putting this together, we
have `A0`, `A1` and `B`, where `A0` is our mask for the lower nybble, `A1` is
our mask for the upper nybble and `B` is our 16 byte block from the haystack:

```ignore
      0x00 0x01 0x02      0x03 ... 0x06      ... 0xF
A0 =  0    0    00000110  0        00000001      0
A1 =  0    0    0         0        00000111      0
B  =  b    a    t         _        t             p
B  =  0x62 0x61 0x74      0x20     0x74          0x70
```

But of course, we can't use `B` with `PSHUFB` yet, since its values are 8 bits,
and we need indexes that are at most 4 bits (corresponding to one of 16
values). We can apply the same transformation to split `B` into lower and upper
nybbles as we did `A`. As before, `B0` corresponds to the lower nybbles and
`B1` corresponds to the upper nybbles:

```ignore
     b   a   t   _   c   a   t   _   f   o   o   _   b   u   m   p
B0 = 0x2 0x1 0x4 0x0 0x3 0x1 0x4 0x0 0x6 0xF 0xF 0x0 0x2 0x5 0xD 0x0
B1 = 0x6 0x6 0x7 0x2 0x6 0x6 0x7 0x2 0x6 0x6 0x6 0x2 0x6 0x7 0x6 0x7
```

And now we have a nice correspondence. `B0` can index `A0` and `B1` can index
`A1`. Here's what we get when we apply `C0 = PSHUFB(A0, B0)`:

```ignore
     b         a        ... f         o         ... p
     A0[0x2]   A0[0x1]      A0[0x6]   A0[0xF]       A0[0x0]
C0 = 00000110  0            00000001  0             0
```

And `C1 = PSHUFB(A1, B1)`:

```ignore
     b         a        ... f         o        ... p
     A1[0x6]   A1[0x6]      A1[0x6]   A1[0x6]      A1[0x7]
C1 = 00000111  00000111     00000111  00000111     0
```

Notice how neither one of `C0` or `C1` is guaranteed to report fully correct
results all on its own. For example, `C1` claims that `b` is a fingerprint for
the pattern `foo` (since `A1[0x6] = 00000111`), and that `o` is a fingerprint
for all of our patterns. But if we combined `C0` and `C1` with an `AND`
operation:

```ignore
     b         a        ... f         o        ... p
C  = 00000110  0            00000001  0            0
```

Then we now have that `C[i]` contains a bitset corresponding to the matching
fingerprints in a haystack's 16 byte block, where `i` is the `ith` byte in that
block.

Once we have that, we can look for the position of the least significant bit
in `C`. (Least significant because we only target little endian here. Thus,
the least significant bytes correspond to bytes in our haystack at a lower
address.) That position, modulo `8`, gives us the pattern that the fingerprint
matches. That position, integer divided by `8`, also gives us the byte offset
that the fingerprint occurs in inside the 16 byte haystack block. Using those
two pieces of information, we can run a verification procedure that tries
to match all substrings containing that fingerprint at that position in the
haystack.


# Implementation notes

The problem with the algorithm as described above is that it uses a single byte
for a fingerprint. This will work well if the fingerprints are rare in the
haystack (e.g., capital letters or special characters in normal English text),
but if the fingerprints are common, you'll wind up spending too much time in
the verification step, which effectively negates the performance benefits of
scanning 16 bytes at a time. Remember, the key to the performance of this
algorithm is to do as little work as possible per 16 (or 32) bytes.

This algorithm can be extrapolated in a relatively straight-forward way to use
larger fingerprints. That is, instead of a single byte prefix, we might use a
two or three byte prefix. The implementation here implements N = {1, 2, 3}
and always picks the largest N possible. The rationale is that the bigger the
fingerprint, the fewer verification steps we'll do. Of course, if N is too
large, then we'll end up doing too much on each step.

The way to extend it is:

1. Add a mask for each byte in the fingerprint. (Remember that each mask is
   composed of two SIMD vectors.) This results in a value of `C` for each byte
   in the fingerprint while searching.
2. When testing each 16 (or 32) byte block, each value of `C` must be shifted
   so that they are aligned. Once aligned, they should all be `AND`'d together.
   This will give you only the bitsets corresponding to the full match of the
   fingerprint. To do this, one needs to save the last byte (for N=2) or last
   two bytes (for N=3) from the previous iteration, and then line them up with
   the first one or two bytes of the next iteration.

## Verification

Verification generally follows the procedure outlined above. The tricky parts
are in the right formulation of operations to get our bits out of our vectors.
We have a limited set of operations available to us on SIMD vectors as 128-bit
or 256-bit numbers, so we wind up needing to rip out 2 (or 4) 64-bit integers
from our vectors, and then run our verification step on each of those. The
verification step looks at the least significant bit set, and from its
position, we can derive the byte offset and bucket. (Again, as described
above.) Once we know the bucket, we do a fairly naive exhaustive search for
every literal in that bucket. (Hyperscan is a bit smarter here and uses a hash
table, but I haven't had time to thoroughly explore that. A few initial
half-hearted attempts resulted in worse performance.)

## AVX

The AVX version of Teddy extrapolates almost perfectly from the SSE version.
The only hickup is that PALIGNR is used to align chunks in the 16-bit version,
and there is no equivalent instruction in AVX. AVX does have VPALIGNR, but it
only works within 128-bit lanes. So there's a bit of tomfoolery to get around
this by shuffling the vectors before calling VPALIGNR.

The only other aspect to AVX is that since our masks are still fundamentally
16-bytes (0x0-0xF), they are duplicated to 32-bytes, so that they can apply to
32-byte chunks.

## Fat Teddy

In the version of Teddy described above, 8 buckets are used to group patterns
that we want to search for. However, when AVX is available, we can extend the
number of buckets to 16 by permitting each byte in our masks to use 16-bits
instead of 8-bits to represent the buckets it belongs to. (This variant is also
in Hyperscan.) However, what we give up is the ability to scan 32 bytes at a
time, even though we're using AVX. Instead, we have to scan 16 bytes at a time.
What we gain, though, is (hopefully) less work in our verification routine.
It patterns are more spread out across more buckets, then there should overall
be fewer false positives. In general, Fat Teddy permits us to grow our capacity
a bit and search for more literals before Teddy gets overwhelmed.

The tricky part of Fat Teddy is in how we adjust our masks and our verification
procedure. For the masks, we simply represent the first 8 buckets in each of
the low 16 bytes, and then the second 8 buckets in each of the high 16 bytes.
Then, in the search loop, instead of loading 32 bytes from the haystack, we
load the same 16 bytes from the haystack into both the low and high 16 byte
portions of our 256-bit vector. So for example, a mask might look like this:

    bits:   00100001 00000000 ... 11000000 00000000 00000001 ... 00000000
    byte:      31       30           16       15       14            0
    offset:    15       14           0        15       14            0
    buckets:  8-15     8-15         8-15      0-7      0-7           0-7

Where `byte` is the position in the vector (higher numbers corresponding to
more significant bits), `offset` is the corresponding position in the haystack
chunk, and `buckets` corresponds to the bucket assignments for that particular
byte.

In particular, notice that the bucket assignments for offset `0` are spread
out between bytes `0` and `16`. This works well for the chunk-by-chunk search
procedure, but verification really wants to process all bucket assignments for
each offset at once. Otherwise, we might wind up finding a match at offset
`1` in one the first 8 buckets, when we really should have reported a match
at offset `0` in one of the second 8 buckets. (Because we want the leftmost
match.)

Thus, for verification, we rearrange the above vector such that it is a
sequence of 16-bit integers, where the least significant 16-bit integer
corresponds to all of the bucket assignments for offset `0`. So with the
above vector, the least significant 16-bit integer would be

    11000000 000000

which was taken from bytes `16` and `0`. Then the verification step pretty much
runs as described, except with 16 buckets instead of 8.


# References

- **[1]** [Hyperscan on GitHub](https://github.com/intel/hyperscan),
    [webpage](https://www.hyperscan.io/)
- **[2a]** Ben-Kiki, O., Bille, P., Breslauer, D., Gasieniec, L., Grossi, R.,
    & Weimann, O. (2011).
    _Optimal packed string matching_.
    In LIPIcs-Leibniz International Proceedings in Informatics (Vol. 13).
    Schloss Dagstuhl-Leibniz-Zentrum fuer Informatik.
    DOI: 10.4230/LIPIcs.FSTTCS.2011.423.
    [PDF](https://drops.dagstuhl.de/opus/volltexte/2011/3355/pdf/37.pdf).
- **[2b]** Ben-Kiki, O., Bille, P., Breslauer, D., Ga̧sieniec, L., Grossi, R.,
    & Weimann, O. (2014).
    _Towards optimal packed string matching_.
    Theoretical Computer Science, 525, 111-129.
    DOI: 10.1016/j.tcs.2013.06.013.
    [PDF](https://www.cs.haifa.ac.il/~oren/Publications/bpsm.pdf).
- **[3]** Bille, P. (2011).
    _Fast searching in packed strings_.
    Journal of Discrete Algorithms, 9(1), 49-56.
    DOI: 10.1016/j.jda.2010.09.003.
    [PDF](https://www.sciencedirect.com/science/article/pii/S1570866710000353).
- **[4a]** Faro, S., & Külekci, M. O. (2012, October).
    _Fast multiple string matching using streaming SIMD extensions technology_.
    In String Processing and Information Retrieval (pp. 217-228).
    Springer Berlin Heidelberg.
    DOI: 10.1007/978-3-642-34109-0_23.
    [PDF](https://www.dmi.unict.it/faro/papers/conference/faro32.pdf).
- **[4b]** Faro, S., & Külekci, M. O. (2013, September).
    _Towards a Very Fast Multiple String Matching Algorithm for Short Patterns_.
    In Stringology (pp. 78-91).
    [PDF](https://www.dmi.unict.it/faro/papers/conference/faro36.pdf).
- **[4c]** Faro, S., & Külekci, M. O. (2013, January).
    _Fast packed string matching for short patterns_.
    In Proceedings of the Meeting on Algorithm Engineering & Expermiments
    (pp. 113-121).
    Society for Industrial and Applied Mathematics.
    [PDF](https://arxiv.org/pdf/1209.6449.pdf).
- **[4d]** Faro, S., & Külekci, M. O. (2014).
    _Fast and flexible packed string matching_.
    Journal of Discrete Algorithms, 28, 61-72.
    DOI: 10.1016/j.jda.2014.07.003.

[1_u]: https://github.com/intel/hyperscan
[5_u]: https://software.intel.com/sites/landingpage/IntrinsicsGuide
