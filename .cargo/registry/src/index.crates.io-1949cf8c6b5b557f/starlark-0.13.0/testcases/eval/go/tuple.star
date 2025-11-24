# @generated
# Copied from https://github.com/google/starlark-go/blob/e81fc95f7bd5bb1495fe69f27c1a99fcc77caa48/starlark/testdata/

# Tests of Starlark 'tuple'

load("asserts.star", "asserts")

# literal
asserts.eq((), ())
asserts.eq((1), 1)
asserts.eq((1,), (1,))
asserts.ne((1), (1,))
asserts.eq((1, 2), (1, 2))
asserts.eq((1, 2, 3, 4, 5), (1, 2, 3, 4, 5))
asserts.ne((1, 2, 3), (1, 2, 4))

# truth
asserts.true((False,))
asserts.true((False, False))
asserts.true(not ())

# indexing, x[i]
asserts.eq(("a", "b")[0], "a")
asserts.eq(("a", "b")[1], "b")

# slicing, x[i:j]
asserts.eq("abcd"[0:4:1], "abcd")
asserts.eq("abcd"[::2], "ac")
asserts.eq("abcd"[1::2], "bd")
asserts.eq("abcd"[4:0:-1], "dcb")
banana = tuple("banana".elems())
asserts.eq(banana[7::-2], tuple("aaa".elems()))
asserts.eq(banana[6::-2], tuple("aaa".elems()))
asserts.eq(banana[5::-2], tuple("aaa".elems()))
asserts.eq(banana[4::-2], tuple("nnb".elems()))

# tuple
asserts.eq(tuple(), ())
asserts.eq(tuple("abc".elems()), ("a", "b", "c"))
asserts.eq(tuple(["a", "b", "c"]), ("a", "b", "c"))
asserts.eq(tuple([1]), (1,))
asserts.fails(lambda: tuple(1), "got int, want iterable")

# tuple * int,  int * tuple
abc = tuple("abc".elems())
asserts.eq(abc * 0, ())
asserts.eq(abc * -1, ())
asserts.eq(abc * 1, abc)
asserts.eq(abc * 3, ("a", "b", "c", "a", "b", "c", "a", "b", "c"))
asserts.eq(0 * abc, ())
asserts.eq(-1 * abc, ())
asserts.eq(1 * abc, abc)
asserts.eq(3 * abc, ("a", "b", "c", "a", "b", "c", "a", "b", "c"))
asserts.fails(lambda: abc * (1000000 * 1000000), "repeat count 1000000000000 too large")
asserts.fails(lambda: abc * 1000000 * 1000000, "excessive repeat \\(3000000 \\* 1000000 elements")

# TODO(adonovan): test use of tuple as sequence
# (for loop, comprehension, library functions).
