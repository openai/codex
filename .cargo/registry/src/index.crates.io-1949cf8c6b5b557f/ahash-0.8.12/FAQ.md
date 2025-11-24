## How does aHash prevent DOS attacks

AHash is designed to [prevent an adversary that does not know the key from being able to create hash collisions or partial collisions.](https://github.com/tkaitchuck/aHash/wiki/How-aHash-is-resists-DOS-attacks)

If you are a cryptographer and would like to help review aHash's algorithm, please post a comment [here](https://github.com/tkaitchuck/aHash/issues/11).

In short, this is achieved by ensuring that:

* aHash is designed to [resist differential crypto analysis](https://github.com/tkaitchuck/aHash/wiki/How-aHash-is-resists-DOS-attacks#differential-analysis). Meaning it should not be possible to devise a scheme to "cancel" out a modification of the internal state from a block of input via some corresponding change in a subsequent block of input.
  * This is achieved by not performing any "premixing" - This reversible mixing gave previous hashes such as murmurhash confidence in their quality, but could be undone by a deliberate attack.
  * Before it is used each chunk of input is "masked" such as by xoring it with an unpredictable value.
* aHash obeys the '[strict avalanche criterion](https://en.wikipedia.org/wiki/Avalanche_effect#Strict_avalanche_criterion)':
Each bit of input has the potential to flip every bit of the output.
* Similarly, each bit in the key can affect every bit in the output.
* Input bits never effect just one, or a very few, bits in intermediate state. This is specifically designed to prevent the sort of 
[differential attacks launched by the sipHash authors](https://emboss.github.io/blog/2012/12/14/breaking-murmur-hash-flooding-dos-reloaded/) which cancel previous inputs.
* The `finish` call at the end of the hash is designed to not expose individual bits of the internal state. 
  * For example in the main algorithm 256bits of state and 256bits of keys are reduced to 64 total bits using 3 rounds of AES encryption. 
Reversing this is more than non-trivial. Most of the information is by definition gone, and any given bit of the internal state is fully diffused across the output.
* In both aHash and its fallback the internal state is divided into two halves which are updated by two unrelated techniques using the same input. - This means that if there is a way to attack one of them it likely won't be able to attack both of them at the same time.
* It is deliberately difficult to 'chain' collisions. (This has been the major technique used to weaponize attacks on other hash functions)

More details are available on [the wiki](https://github.com/tkaitchuck/aHash/wiki/How-aHash-is-resists-DOS-attacks).

## Why not use a cryptographic hash in a hashmap.

Cryptographic hashes are designed to make is nearly impossible to find two items that collide when the attacker has full control
over the input. This has several implications:

* They are very difficult to construct, and have to go to a lot of effort to ensure that collisions are not possible.
* They have no notion of a 'key'. Rather, they are fully deterministic and provide exactly one hash for a given input.

For a HashMap the requirements are different.

* Speed is very important, especially for short inputs. Often the key for a HashMap is a single `u32` or similar, and to be effective
the bucket that it should be hashed to needs to be computed in just a few CPU cycles.
* A hashmap does not need to provide a hard and fast guarantee that no two inputs will ever collide. Hence, hashCodes are not 256bits 
but are just 64 or 32 bits in length. Often the first thing done with the hashcode is to truncate it further to compute which among a few buckets should be used for a key. 
  * Here collisions are expected, and a cheap to deal with provided there is no systematic way to generated huge numbers of values that all
go to the same bucket.
  * This also means that unlike a cryptographic hash partial collisions matter. It doesn't do a hashmap any good to produce a unique 256bit hash if
the lower 12 bits are all the same. This means that even a provably irreversible hash would not offer protection from a DOS attack in a hashmap
because an attacker can easily just brute force the bottom N bits.

From a cryptography point of view, a hashmap needs something closer to a block cypher.
Where the input can be quickly mixed in a way that cannot be reversed without knowing a key.

## Why isn't aHash cryptographically secure

It is not designed to be. 
Attempting to use aHash as a secure hash will likely fail to hold up for several reasons:

1. aHash relies on random keys which are assumed to not be observable by an attacker. For a cryptographic hash all inputs can be seen and controlled by the attacker.
2. aHash has not yet gone through peer review, which is a pre-requisite for security critical algorithms.
3. Because aHash uses reduced rounds of AES as opposed to the standard of 10. Things like the SQUARE attack apply to part of the internal state.
(These are mitigated by other means to prevent producing collections, but would be a problem in other contexts).
4. Like any cypher based hash, it will show certain statistical deviations from truly random output when comparing a (VERY) large number of hashes. 
(By definition cyphers have fewer collisions than truly random data.)

There are efforts to build a secure hash function that uses AES-NI for acceleration, but aHash is not one of them.

## How is aHash so fast

AHash uses a number of tricks. 

One trick is taking advantage of specialization. If aHash is compiled on nightly it will take
advantage of specialized hash implementations for strings, slices, and primitives. 

Another is taking advantage of hardware instructions.
When it is available aHash uses AES rounds using the AES-NI instruction. AES-NI is very fast (on an intel i7-6700 it 
is as fast as a 64 bit multiplication.) and handles 16 bytes of input at a time, while being a very strong permutation.

This is obviously much faster than most standard approaches to hashing, and does a better job of scrambling data than most non-secure hashes.

On an intel i7-6700 compiled on nightly Rust with flags `-C opt-level=3 -C target-cpu=native -C codegen-units=1`:

| Input   | SipHash 1-3 time | FnvHash time|FxHash time| aHash time| aHash Fallback* |
|----------------|-----------|-----------|-----------|-----------|---------------|
| u8             | 9.3271 ns | 0.808 ns  | **0.594 ns**  | 0.7704 ns | 0.7664 ns |
| u16            | 9.5139 ns | 0.803 ns  | **0.594 ns**  | 0.7653 ns | 0.7704 ns |
| u32            | 9.1196 ns | 1.4424 ns | **0.594 ns**  | 0.7637 ns | 0.7712 ns |
| u64            | 10.854 ns | 3.0484 ns | **0.628 ns**  | 0.7788 ns | 0.7888 ns |
| u128           | 12.465 ns | 7.0728 ns | 0.799 ns  | **0.6174 ns** | 0.6250 ns |
| 1 byte string  | 11.745 ns | 2.4743 ns | 2.4000 ns | **1.4921 ns** | 1.5861 ns |
| 3 byte string  | 12.066 ns | 3.5221 ns | 2.9253 ns | **1.4745 ns** | 1.8518 ns |
| 4 byte string  | 11.634 ns | 4.0770 ns | 1.8818 ns | **1.5206 ns** | 1.8924 ns |
| 7 byte string  | 14.762 ns | 5.9780 ns | 3.2282 ns | **1.5207 ns** | 1.8933 ns |
| 8 byte string  | 13.442 ns | 4.0535 ns | 2.9422 ns | **1.6262 ns** | 1.8929 ns |
| 15 byte string | 16.880 ns | 8.3434 ns | 4.6070 ns | **1.6265 ns** | 1.7965 ns |
| 16 byte string | 15.155 ns | 7.5796 ns | 3.2619 ns | **1.6262 ns** | 1.8011 ns |
| 24 byte string | 16.521 ns | 12.492 ns | 3.5424 ns | **1.6266 ns** | 2.8311 ns |
| 68 byte string | 24.598 ns | 50.715 ns | 5.8312 ns | **4.8282 ns** | 5.4824 ns |
| 132 byte string| 39.224 ns | 119.96 ns | 11.777 ns | **6.5087 ns** | 9.1459 ns |
|1024 byte string| 254.00 ns | 1087.3 ns | 156.41 ns | **25.402 ns** | 54.566 ns |

* Fallback refers to the algorithm aHash would use if AES instructions are unavailable.
For reference a hash that does nothing (not even reads the input data takes) **0.520 ns**. So that represents the fastest
possible time.

As you can see above aHash like `FxHash` provides a large speedup over `SipHash-1-3` which is already nearly twice as fast as `SipHash-2-4`.

Rust's HashMap by default uses `SipHash-1-3` because faster hash functions such as `FxHash` are predictable and vulnerable to denial of
service attacks. While `aHash` has both very strong scrambling and very high performance.

AHash performs well when dealing with large inputs because aHash reads 8 or 16 bytes at a time. (depending on availability of AES-NI)

Because of this, and its optimized logic, `aHash` is able to outperform `FxHash` with strings.
It also provides especially good performance dealing with unaligned input.
(Notice the big performance gaps between 3 vs 4, 7 vs 8 and 15 vs 16 in `FxHash` above)

### Which CPUs can use the hardware acceleration

Hardware AES instructions are built into Intel processors built after 2010 and AMD processors after 2012.
It is also available on [many other CPUs](https://en.wikipedia.org/wiki/AES_instruction_set) should in eventually
be able to get aHash to work. However, only X86 and X86-64 are the only supported architectures at the moment, as currently
they are the only architectures for which Rust provides an intrinsic.

aHash also uses `sse2` and `sse3` instructions. X86 processors that have `aesni` also have these instruction sets.
