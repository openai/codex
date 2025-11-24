# @generated
# Copied from https://github.com/google/starlark-go/blob/87f333178d5942de51b193111d6f636c79833ea5/starlark/testdata/

# Tests of Starlark 'float'
# option:set

load("asserts.star", "asserts")

# TODO(adonovan): more tests:
# - precision
# - limits

# type
asserts.eq(type(0.0), "float")

# truth
asserts.true(123.0)
asserts.true(-1.0)
asserts.true(not 0.0)
asserts.true(-1.0e-45)
asserts.true(float("NaN"))

# not iterable
asserts.fails(lambda: len(0.0), 'has no len')
asserts.fails(lambda: [x for x in 0.0], 'float value is not iterable')

# literals
asserts.eq(type(1.234), "float")
asserts.eq(type(1e10), "float")
asserts.eq(type(1e+10), "float")
asserts.eq(type(1e-10), "float")
asserts.eq(type(1.234e10), "float")
asserts.eq(type(1.234e+10), "float")
asserts.eq(type(1.234e-10), "float")

# int/float equality
asserts.eq(0.0, 0)
asserts.eq(0, 0.0)
asserts.eq(1.0, 1)
asserts.eq(1, 1.0)
asserts.true(1.23e45 != 1229999999999999973814869011019624571608236031)
asserts.true(1.23e45 == 1229999999999999973814869011019624571608236032)
asserts.true(1.23e45 != 1229999999999999973814869011019624571608236033)
asserts.true(1229999999999999973814869011019624571608236031 != 1.23e45)
asserts.true(1229999999999999973814869011019624571608236032 == 1.23e45)
asserts.true(1229999999999999973814869011019624571608236033 != 1.23e45)

# loss of precision
p53 = 1<<53
asserts.eq(float(p53-1), p53-1)
asserts.eq(float(p53+0), p53+0)
asserts.eq(float(p53+1), p53+0) #
asserts.eq(float(p53+2), p53+2)
asserts.eq(float(p53+3), p53+4) #
asserts.eq(float(p53+4), p53+4)
asserts.eq(float(p53+5), p53+4) #
asserts.eq(float(p53+6), p53+6)
asserts.eq(float(p53+7), p53+8) #
asserts.eq(float(p53+8), p53+8)

# Regression test for https://github.com/google/starlark-go/issues/375.
maxint64 = (1<<63)-1
asserts.eq(int(float(maxint64)), 9223372036854775808)

asserts.true(float(p53+1) != p53+1) # comparisons are exact
asserts.eq(float(p53+1) - (p53+1), 0) # arithmetic entails rounding

asserts.fails(lambda: {123.0: "f", 123: "i"}, "duplicate key: 123")

# equal int/float values have same hash
d = {123.0: "x"}
d[123] = "y"
asserts.eq(len(d), 1)
asserts.eq(d[123.0], "y")

# literals (mostly covered by scanner tests)
asserts.eq(str(0.), "0.0")
asserts.eq(str(.0), "0.0")
asserts.true(5.0 != 4.999999999999999)
asserts.eq(5.0, 4.9999999999999999) # both literals denote 5.0
asserts.eq(1.23e45, 1.23 * 1000000000000000000000000000000000000000000000)
asserts.eq(str(1.23e-45 - (1.23 / 1000000000000000000000000000000000000000000000)), "-1.5557538194652854e-61")

nan = float("NaN")
inf = float("+Inf")
neginf = float("-Inf")
negzero = (-1e-323 / 10)

# -- arithmetic --

# +float, -float
asserts.eq(+(123.0), 123.0)
asserts.eq(-(123.0), -123.0)
asserts.eq(-(-(123.0)), 123.0)
asserts.eq(+(inf), inf)
asserts.eq(-(inf), neginf)
asserts.eq(-(neginf), inf)
asserts.eq(str(-(nan)), "nan")
# +
asserts.eq(1.2e3 + 5.6e7, 5.60012e+07)
asserts.eq(1.2e3 + 1, 1201)
asserts.eq(1 + 1.2e3, 1201)
asserts.eq(str(1.2e3 + nan), "nan")
asserts.eq(inf + 0, inf)
asserts.eq(inf + 1, inf)
asserts.eq(inf + inf, inf)
asserts.eq(str(inf + neginf), "nan")
# -
asserts.eq(1.2e3 - 5.6e7, -5.59988e+07)
asserts.eq(1.2e3 - 1, 1199)
asserts.eq(1 - 1.2e3, -1199)
asserts.eq(str(1.2e3 - nan), "nan")
asserts.eq(inf - 0, inf)
asserts.eq(inf - 1, inf)
asserts.eq(str(inf - inf), "nan")
asserts.eq(inf - neginf, inf)
# *
asserts.eq(1.5e6 * 2.2e3, 3.3e9)
asserts.eq(1.5e6 * 123, 1.845e+08)
asserts.eq(123 * 1.5e6, 1.845e+08)
asserts.eq(str(1.2e3 * nan), "nan")
asserts.eq(str(inf * 0), "nan")
asserts.eq(inf * 1, inf)
asserts.eq(inf * inf, inf)
asserts.eq(inf * neginf, neginf)
# %
asserts.eq(100.0 % 7.0, 2)
asserts.eq(100.0 % -7.0, -5) # NB: different from Go / Java
asserts.eq(-100.0 % 7.0, 5) # NB: different from Go / Java
asserts.eq(-100.0 % -7.0, -2)
asserts.eq(-100.0 % 7, 5)
asserts.eq(100 % 7.0, 2)
asserts.eq(str(1.2e3 % nan), "nan")
asserts.eq(str(inf % 1), "nan")
asserts.eq(str(inf % inf), "nan")
asserts.eq(str(inf % neginf), "nan")
# /
asserts.eq(str(100.0 / 7.0), "14.285714285714286")
asserts.eq(str(100 / 7.0), "14.285714285714286")
asserts.eq(str(100.0 / 7), "14.285714285714286")
asserts.eq(str(100.0 / nan), "nan")
# //
asserts.eq(100.0 // 7.0, 14)
asserts.eq(100 // 7.0, 14)
asserts.eq(100.0 // 7, 14)
asserts.eq(100.0 // -7.0, -15)
asserts.eq(100 // -7.0, -15)
asserts.eq(100.0 // -7, -15)
asserts.eq(str(1 // neginf), "-0.0")
asserts.eq(str(100.0 // nan), "nan")

# addition
asserts.eq(0.0 + 1.0, 1.0)
asserts.eq(1.0 + 1.0, 2.0)
asserts.eq(1.25 + 2.75, 4.0)
asserts.eq(5.0 + 7.0, 12.0)
asserts.eq(5.1 + 7, 12.1)  # float + int
asserts.eq(7 + 5.1, 12.1)  # int + float

# subtraction
asserts.eq(5.0 - 7.0, -2.0)
asserts.eq(5.1 - 7.1, -2.0)
asserts.eq(5.5 - 7, -1.5)
asserts.eq(5 - 7.5, -2.5)
asserts.eq(0.0 - 1.0, -1.0)

# multiplication
asserts.eq(5.0 * 7.0, 35.0)
asserts.eq(5.5 * 2.5, 13.75)
asserts.eq(5.5 * 7, 38.5)
asserts.eq(5 * 7.1, 35.5)

# real division (like Python 3)
# The / operator is available only when the 'fp' dialect option is enabled.
asserts.eq(100.0 / 8.0, 12.5)
asserts.eq(100.0 / -8.0, -12.5)
asserts.eq(-100.0 / 8.0, -12.5)
asserts.eq(-100.0 / -8.0, 12.5)
asserts.eq(98.0 / 8.0, 12.25)
asserts.eq(98.0 / -8.0, -12.25)
asserts.eq(-98.0 / 8.0, -12.25)
asserts.eq(-98.0 / -8.0, 12.25)
asserts.eq(2.5 / 2.0, 1.25)
asserts.eq(2.5 / 2, 1.25)
asserts.eq(5 / 4.0, 1.25)
asserts.eq(5 / 4, 1.25)
asserts.fails(lambda: 1.0 / 0, "floating-point division by zero")
asserts.fails(lambda: 1.0 / 0.0, "floating-point division by zero")
asserts.fails(lambda: 1 / 0.0, "floating-point division by zero")

# floored division
asserts.eq(100.0 // 8.0, 12.0)
asserts.eq(100.0 // -8.0, -13.0)
asserts.eq(-100.0 // 8.0, -13.0)
asserts.eq(-100.0 // -8.0, 12.0)
asserts.eq(98.0 // 8.0, 12.0)
asserts.eq(98.0 // -8.0, -13.0)
asserts.eq(-98.0 // 8.0, -13.0)
asserts.eq(-98.0 // -8.0, 12.0)
asserts.eq(2.5 // 2.0, 1.0)
asserts.eq(2.5 // 2, 1.0)
asserts.eq(5 // 4.0, 1.0)
asserts.eq(5 // 4, 1)
asserts.eq(type(5 // 4), "int")
asserts.fails(lambda: 1.0 // 0, "floored division by zero")
asserts.fails(lambda: 1.0 // 0.0, "floored division by zero")
asserts.fails(lambda: 1 // 0.0, "floored division by zero")

# remainder
asserts.eq(100.0 % 8.0, 4.0)
asserts.eq(100.0 % -8.0, -4.0)
asserts.eq(-100.0 % 8.0, 4.0)
asserts.eq(-100.0 % -8.0, -4.0)
asserts.eq(98.0 % 8.0, 2.0)
asserts.eq(98.0 % -8.0, -6.0)
asserts.eq(-98.0 % 8.0, 6.0)
asserts.eq(-98.0 % -8.0, -2.0)
asserts.eq(2.5 % 2.0, 0.5)
asserts.eq(2.5 % 2, 0.5)
asserts.eq(5 % 4.0, 1.0)
asserts.fails(lambda: 1.0 % 0, "floating-point modulo by zero")
asserts.fails(lambda: 1.0 % 0.0, "floating-point modulo by zero")
asserts.fails(lambda: 1 % 0.0, "floating-point modulo by zero")

# floats cannot be used as indices, even if integral
asserts.fails(lambda: "abc"[1.0], "want int")
asserts.fails(lambda: ["A", "B", "C"].insert(1.0, "D"), "want int")
asserts.fails(lambda: range(3)[1.0], "got float, want int")

# -- comparisons --
# NaN
asserts.true(nan == nan) # \
asserts.true(nan >= nan) #  unlike Python
asserts.true(nan <= nan) # /
asserts.true(not (nan > nan))
asserts.true(not (nan < nan))
asserts.true(not (nan != nan)) # unlike Python
# Sort is stable: 0.0 and -0.0 are equal, but they are not permuted.
# Similarly 1 and 1.0.
asserts.eq(
    str(sorted([inf, neginf, nan, 1e300, -1e300, 1.0, -1.0, 1, -1, 1e-300, -1e-300, 0, 0.0, negzero, 1e-300, -1e-300])),
    "[-inf, -1e+300, -1.0, -1, -1e-300, -1e-300, 0, 0.0, -0.0, 1e-300, 1e-300, 1.0, 1, 1e+300, +inf, nan]")

# Sort is stable, and its result contains no adjacent x, y such that y > x.
# Note: Python's reverse sort is unstable; see https://bugs.python.org/issue36095.
asserts.eq(str(sorted([7, 3, nan, 1, 9])), "[1, 3, 7, 9, nan]")
asserts.eq(str(sorted([7, 3, nan, 1, 9], reverse=True)), "[nan, 9, 7, 3, 1]")

# All NaN values compare equal. (Identical objects compare equal.)
nandict = {nan: 1}
nandict[nan] = 2
asserts.eq(len(nandict), 1) # (same as Python)
asserts.eq(nandict[nan], 2) # (same as Python)
asserts.fails(lambda: {nan: 1, nan: 2}, "duplicate key: nan")

nandict[float('nan')] = 3 # a distinct NaN object
asserts.eq(str(nandict), "{nan: 3}") # (Python: {nan: 2, nan: 3})

asserts.eq(str({inf: 1, neginf: 2}), "{+inf: 1, -inf: 2}")

# zero
asserts.eq(0.0, negzero)

# inf
asserts.eq(+inf / +inf, nan)
asserts.eq(+inf / -inf, nan)
asserts.eq(-inf / +inf, nan)
asserts.eq(0.0 / +inf, 0.0)
asserts.eq(0.0 / -inf, 0.0)
asserts.true(inf > -inf)
asserts.eq(inf, -neginf)
# TODO(adonovan): assert inf > any finite number, etc.

# negative zero
negz = -0
asserts.eq(negz, 0)

# min/max ordering with NaN (the greatest float value)
asserts.eq(max([1, nan, 3]), nan)
asserts.eq(max([nan, 2, 3]), nan)
asserts.eq(min([1, nan, 3]), 1)
asserts.eq(min([nan, 2, 3]), 2)

# float/float comparisons
fltmax = 1.7976931348623157e+308 # approx
fltmin = 4.9406564584124654e-324 # approx
asserts.lt(-inf, -fltmax)
asserts.lt(-fltmax, -1.0)
asserts.lt(-1.0, -fltmin)
asserts.lt(-fltmin, 0.0)
asserts.lt(0, fltmin)
asserts.lt(fltmin, 1.0)
asserts.lt(1.0, fltmax)
asserts.lt(fltmax, inf)

# int/float comparisons
asserts.eq(0, 0.0)
asserts.eq(1, 1.0)
asserts.eq(-1, -1.0)
asserts.ne(-1, -1.0 + 1e-7)
asserts.lt(-2, -2 + 1e-15)

# int conversion (rounds towards zero)
asserts.eq(int(100.1), 100)
asserts.eq(int(100.0), 100)
asserts.eq(int(99.9), 99)
asserts.eq(int(-99.9), -99)
asserts.eq(int(-100.0), -100)
asserts.eq(int(-100.1), -100)
asserts.eq(int(1e100), int("10000000000000000159028911097599180468360808563945281389781327557747838772170381060813469985856815104"))
asserts.fails(lambda: int(inf), "cannot convert.*infinity")
asserts.fails(lambda: int(nan), "cannot convert.*NaN")

# -- float() function --
asserts.eq(float(), 0.0)
# float(bool)
asserts.eq(float(False), 0.0)
asserts.eq(float(True), 1.0)
# float(int)
asserts.eq(float(0), 0.0)
asserts.eq(float(1), 1.0)
asserts.eq(float(123), 123.0)
asserts.eq(float(123 * 1000000 * 1000000 * 1000000 * 1000000 * 1000000), 1.23e+32)
# float(float)
asserts.eq(float(1.1), 1.1)
asserts.fails(lambda: float(None), "want number or string")
asserts.ne(False, 0.0) # differs from Python
asserts.ne(True, 1.0)
# float(string)
asserts.eq(float("1.1"), 1.1)
asserts.fails(lambda: float("1.1abc"), "invalid float literal")
asserts.fails(lambda: float("1e100.0"), "invalid float literal")
asserts.fails(lambda: float("1e1000"), "floating-point number too large")
asserts.eq(float("-1.1"), -1.1)
asserts.eq(float("+1.1"), +1.1)
asserts.eq(float("+Inf"), inf)
asserts.eq(float("-Inf"), neginf)
asserts.eq(float("NaN"), nan)
asserts.eq(float("NaN"), nan)
asserts.eq(float("+NAN"), nan)
asserts.eq(float("-nan"), nan)
asserts.eq(str(float("Inf")), "+inf")
asserts.eq(str(float("+INF")), "+inf")
asserts.eq(str(float("-inf")), "-inf")
asserts.eq(str(float("+InFiniTy")), "+inf")
asserts.eq(str(float("-iNFiniTy")), "-inf")
asserts.fails(lambda: float("one point two"), "invalid float literal: one point two")
asserts.fails(lambda: float("1.2.3"), "invalid float literal: 1.2.3")
asserts.fails(lambda: float(123 << 500 << 500 << 50), "int too large to convert to float")
asserts.fails(lambda: float(-123 << 500 << 500 << 50), "int too large to convert to float")
asserts.fails(lambda: float(str(-123 << 500 << 500 << 50)), "floating-point number too large")

# -- implicit float(int) conversions --
asserts.fails(lambda: (1<<500<<500<<500) + 0.0, "int too large to convert to float")
asserts.fails(lambda: 0.0 + (1<<500<<500<<500), "int too large to convert to float")
asserts.fails(lambda: (1<<500<<500<<500) - 0.0, "int too large to convert to float")
asserts.fails(lambda: 0.0 - (1<<500<<500<<500), "int too large to convert to float")
asserts.fails(lambda: (1<<500<<500<<500) * 1.0, "int too large to convert to float")
asserts.fails(lambda: 1.0 * (1<<500<<500<<500), "int too large to convert to float")
asserts.fails(lambda: (1<<500<<500<<500) / 1.0, "int too large to convert to float")
asserts.fails(lambda: 1.0 / (1<<500<<500<<500), "int too large to convert to float")
asserts.fails(lambda: (1<<500<<500<<500) // 1.0, "int too large to convert to float")
asserts.fails(lambda: 1.0 // (1<<500<<500<<500), "int too large to convert to float")
asserts.fails(lambda: (1<<500<<500<<500) % 1.0, "int too large to convert to float")
asserts.fails(lambda: 1.0 % (1<<500<<500<<500), "int too large to convert to float")


# -- int function --
asserts.eq(int(0.0), 0)
asserts.eq(int(1.0), 1)
asserts.eq(int(1.1), 1)
asserts.eq(int(0.9), 0)
asserts.eq(int(-1.1), -1.0)
asserts.eq(int(-1.0), -1.0)
asserts.eq(int(-0.9), 0.0)
asserts.eq(int(1.23e+32), 123000000000000004979083645550592)
asserts.eq(int(-1.23e-32), 0)
asserts.eq(int(1.23e-32), 0)
asserts.fails(lambda: int(float("+Inf")), "cannot convert float infinity to integer")
asserts.fails(lambda: int(float("-Inf")), "cannot convert float infinity to integer")
asserts.fails(lambda: int(float("NaN")), "cannot convert float NaN to integer")


# hash
# Check that equal float and int values have the same internal hash.
def checkhash():
  for a in [
            1.23e100,   # int overflow in starlark-rust
            1.23e10,    # int overflow in starlark-rust
            1.23e1, 1.23, 1,
            4294967295, 8589934591, 9223372036854775807, # int overflow in starlark-rust
            ]:
    for b in [a, -a, 1/a, -1/a]:
      f = float(b)
      i = int(b)
      if f == i:
        fh = {f: None}
        ih = {i: None}
        if fh != ih:
          asserts.true(False, "{%v: None} != {%v: None}: hashes vary" % fh, ih)
checkhash()

# string formatting

# %d
asserts.eq("%d" % 0, "0")
asserts.eq("%d" % 0.0, "0")
asserts.eq("%d" % 123, "123")
asserts.eq("%d" % 123.0, "123")
asserts.eq("%d" % 1.23e45, "1229999999999999973814869011019624571608236032")
# (see below for '%d' % NaN/Inf)
asserts.eq("%d" % negzero, "0")
asserts.fails(lambda: "%d" % float("NaN"), "cannot convert float NaN to integer")
asserts.fails(lambda: "%d" % float("+Inf"), "cannot convert float infinity to integer")
asserts.fails(lambda: "%d" % float("-Inf"), "cannot convert float infinity to integer")

# %e
asserts.eq("%e" % 0, "0.000000e+00")
asserts.eq("%e" % 0.0, "0.000000e+00")
asserts.eq("%e" % 123, "1.230000e+02")
asserts.eq("%e" % 123.0, "1.230000e+02")
asserts.eq("%e" % 1.23e45, "1.230000e+45")
asserts.eq("%e" % -1.23e-45, "-1.230000e-45")
asserts.eq("%e" % nan, "nan")
asserts.eq("%e" % inf, "+inf")
asserts.eq("%e" % neginf, "-inf")
asserts.eq("%e" % negzero, "-0.000000e+00")
asserts.fails(lambda: "%e" % "123", "requires float, not str")
# %f
asserts.eq("%f" % 0, "0.000000")
asserts.eq("%f" % 0.0, "0.000000")
asserts.eq("%f" % 123, "123.000000")
asserts.eq("%f" % 123.0, "123.000000")
# Note: Starlark/Java emits 1230000000000000000000000000000000000000000000.000000. Why?
asserts.eq("%f" % 1.23e45, "1229999999999999973814869011019624571608236032.000000")
asserts.eq("%f" % -1.23e-45, "-0.000000")
asserts.eq("%f" % nan, "nan")
asserts.eq("%f" % inf, "+inf")
asserts.eq("%f" % neginf, "-inf")
asserts.eq("%f" % negzero, "-0.000000")
asserts.fails(lambda: "%f" % "123", "requires float, not str")
# %g
asserts.eq("%g" % 0, "0.0")
asserts.eq("%g" % 0.0, "0.0")
asserts.eq("%g" % 123, "123.0")
asserts.eq("%g" % 123.0, "123.0")
asserts.eq("%g" % 1.110, "1.11")
asserts.eq("%g" % 1e5, "100000.0")
asserts.eq("%g" % 1e6, "1e+06") # Note: threshold of scientific notation is 1e17 in Starlark/Java
asserts.eq("%g" % 1.23e45, "1.23e+45")
asserts.eq("%g" % -1.23e-45, "-1.23e-45")
asserts.eq("%g" % nan, "nan")
asserts.eq("%g" % inf, "+inf")
asserts.eq("%g" % neginf, "-inf")
asserts.eq("%g" % negzero, "-0.0")
# str uses %g
asserts.eq(str(0.0), "0.0")
asserts.eq(str(123.0), "123.0")
asserts.eq(str(1.23e45), "1.23e+45")
asserts.eq(str(-1.23e-45), "-1.23e-45")
asserts.eq(str(nan), "nan")
asserts.eq(str(inf), "+inf")
asserts.eq(str(neginf), "-inf")
asserts.eq(str(negzero), "-0.0")
asserts.fails(lambda: "%g" % "123", "requires float, not str")

i0 = 1
f0 = 1.0
asserts.eq(type(i0), "int")
asserts.eq(type(f0), "float")

ops = {
    '+': lambda x, y: x + y,
    '-': lambda x, y: x - y,
    '*': lambda x, y: x * y,
    '/': lambda x, y: x / y,
    '//': lambda x, y: x // y,
    '%': lambda x, y: x % y,
}

# Check that if either argument is a float, so too is the result.
def checktypes():
  want = set("""
int + int = int
int + float = float
float + int = float
float + float = float
int - int = int
int - float = float
float - int = float
float - float = float
int * int = int
int * float = float
float * int = float
float * float = float
int / int = float
int / float = float
float / int = float
float / float = float
int // int = int
int // float = float
float // int = float
float // float = float
int % int = int
int % float = float
float % int = float
float % float = float
"""[1:].splitlines())
  for opname in ("+", "-", "*", "/", "%"):
    for x in [i0, f0]:
      for y in [i0, f0]:
        op = ops[opname]
        got = "%s %s %s = %s" % (type(x), opname, type(y), type(op(x, y)))
        asserts.contains(want, got)
checktypes()
