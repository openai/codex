# @generated
# Copied from https://github.com/google/starlark-go/blob/e81fc95f7bd5bb1495fe69f27c1a99fcc77caa48/starlark/testdata/

# Tests of Starlark 'function'
# option:nesteddef option:set

# TODO(adonovan):
# - add some introspection functions for looking at function values
#   and test that functions have correct position, free vars, names of locals, etc.
# - move the hard-coded tests of parameter passing from eval_test.go to here.

load("asserts.star", "asserts", "freeze")

hf = hasfields()
hf.x = [len]
asserts.eq(hf.x[0]("abc"), 3)
def f():
   return lambda: 1
asserts.eq(f()(), 1)
asserts.eq(["abc"][0][0].upper(), "A")

# functions may be recursively defined,
# so long as they don't dynamically recur.
calls = []
def yin(x):
  calls.append("yin")
  if x:
    yang(False)

def yang(x):
  calls.append("yang")
  if x:
    yin(False)

yin(True)
asserts.eq(calls, ["yin", "yang"])

calls.clear()
yang(True)
asserts.eq(calls, ["yang", "yin"])


# builtin_function_or_method use identity equivalence.
closures = set(["".count for _ in range(10)])
# asserts.eq(len(closures), 10)

---
# Default values of function parameters are mutable.
load("asserts.star", "asserts", "freeze")

def f(x=[0]):
  return x

asserts.eq(f(), [0])

f().append(1)
asserts.eq(f(), [0, 1])

# Freezing a function value freezes its parameter defaults.
freeze(f)
asserts.fails(lambda: f().append(2), "cannot append to frozen list")

---
# This is a well known corner case of parsing in Python.
load("asserts.star", "asserts")

f = lambda x: 1 if x else 0
asserts.eq(f(True), 1)
asserts.eq(f(False), 0)

x = True
f2 = (lambda x: 1) if x else 0
asserts.eq(f2(123), 1)

tf = lambda: True, lambda: False
asserts.true(tf[0]())
asserts.true(not tf[1]())

---
# Missing parameters are correctly reported
# in functions of more than 64 parameters.
# (This tests a corner case of the implementation:
# we avoid a map allocation for <64 parameters)

load("asserts.star", "asserts")

def f(a, b, c, d, e, f, g, h,
      i, j, k, l, m, n, o, p,
      q, r, s, t, u, v, w, x,
      y, z, A, B, C, D, E, F,
      G, H, I, J, K, L, M, N,
      O, P, Q, R, S, T, U, V,
      W, X, Y, Z, aa, bb, cc, dd,
      ee, ff, gg, hh, ii, jj, kk, ll,
      mm):
  pass

asserts.fails(lambda: f(
    1, 2, 3, 4, 5, 6, 7, 8,
    9, 10, 11, 12, 13, 14, 15, 16,
    17, 18, 19, 20, 21, 22, 23, 24,
    25, 26, 27, 28, 29, 30, 31, 32,
    33, 34, 35, 36, 37, 38, 39, 40,
    41, 42, 43, 44, 45, 46, 47, 48,
    49, 50, 51, 52, 53, 54, 55, 56,
    57, 58, 59, 60, 61, 62, 63, 64), "missing 1 argument \\(mm\\)")

asserts.fails(lambda: f(
    1, 2, 3, 4, 5, 6, 7, 8,
    9, 10, 11, 12, 13, 14, 15, 16,
    17, 18, 19, 20, 21, 22, 23, 24,
    25, 26, 27, 28, 29, 30, 31, 32,
    33, 34, 35, 36, 37, 38, 39, 40,
    41, 42, 43, 44, 45, 46, 47, 48,
    49, 50, 51, 52, 53, 54, 55, 56,
    57, 58, 59, 60, 61, 62, 63, 64, 65,
    mm = 100), 'multiple values for parameter "mm"')

---
# Regression test for github.com/google/starlark-go/issues/21,
# which concerns dynamic checks.
# Related: https://github.com/bazelbuild/starlark/issues/21,
# which concerns static checks.

load("asserts.star", "asserts")

def f(*args, **kwargs):
  return args, kwargs

asserts.eq(f(x=1, y=2), ((), {"x": 1, "y": 2}))
asserts.fails(lambda: f(x=1, **dict(x=2)), 'multiple values for parameter "x"')

def g(x, y):
  return x, y

asserts.eq(g(1, y=2), (1, 2))
asserts.fails(lambda: g(1, y=2, **{'y': 3}), 'multiple values for parameter "y"')

---
# Regression test for a bug in CALL_VAR_KW.

load("asserts.star", "asserts")

def f(a, b, x, y):
  return a+b+x+y

asserts.eq(f(*("a", "b"), **dict(y="y", x="x")) + ".", 'abxy.')
---
# Order of evaluation of function arguments.
# Regression test for github.com/google/skylark/issues/135.
load("asserts.star", "asserts")

r = []

def id(x):
  r.append(x)
  return x

def f(*args, **kwargs):
  return (args, kwargs)

y = f(id(1), id(2), x=id(3), *[id(4)], **dict(z=id(5)))
asserts.eq(y, ((1, 2, 4), dict(x=3, z=5)))

# This matches Python2 and Starlark-in-Java, but not Python3 [1 2 4 3 6].
# *args and *kwargs are evaluated last.
# (Python[23] also allows keyword arguments after *args.)
# See github.com/bazelbuild/starlark#13 for spec change.
asserts.eq(r, [1, 2, 3, 4, 5])

---
# option:nesteddef option:recursion
# See github.com/bazelbuild/starlark#170
load("asserts.star", "asserts")

def a():
    list = []
    def b(n):
        list.append(n)
        if n > 0:
            b(n - 1) # recursive reference to b

    b(3)
    return list

asserts.eq(a(), [3, 2, 1, 0])

def c():
    list = []
    x = 1
    def d():
      list.append(x) # this use of x observes both assignments
    d()
    x = 2
    d()
    return list

asserts.eq(c(), [1, 2])

def e():
    def f():
      return x # forward reference ok: x is a closure cell
    x = 1
    return f()

asserts.eq(e(), 1)

---
# option:nesteddef
load("asserts.star", "asserts")

def e():
    x = 1
    def f():
      print(x) # this reference to x fails
      x = 3    # because this assignment makes x local to f
    f()

asserts.fails(e, "local variable x referenced before assignment")


---
# A trailing comma is allowed in any function definition or call.
# This reduces the need to edit neighboring lines when editing defs
# or calls splayed across multiple lines.

def a(x,): pass
def b(x, y=None, ): pass
def c(x, y=None, *args, ): pass
def d(x, y=None, *args, z=None, ): pass
def e(x, y=None, *args, z=None, **kwargs, ): pass

a(1,)
b(1, y=2, )
#c(1, *[], )
#d(1, *[], z=None, )
#e(1, *[], z=None, *{}, )
