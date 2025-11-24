# @generated
# Copied from https://github.com/google/starlark-go/blob/e81fc95f7bd5bb1495fe69f27c1a99fcc77caa48/starlark/testdata/

# Tests of Starlark assignment.

# This is a "chunked" file: each "---" effectively starts a new file.

# tuple assignment
load("asserts.star", "asserts")

() = () # empty ok

a, b, c = 1, 2, 3
asserts.eq(a, 1)
asserts.eq(b, 2)
asserts.eq(c, 3)

(d, e, f,) = (1, 2, 3) # trailing comma ok
---
(a, b, c) = 1 ### "got int in sequence assignment"
---
(a, b) = () ### "too few values to unpack"
---
(a, b) = (1,) ### "too few values to unpack"
---
(a, b, c) = (1, 2) ### "too few values to unpack"
---
(a, b) = (1, 2, 3) ### "too many values to unpack"
---
() = 1 ### "got int in sequence assignment"
---
() = (1,) ### "too many values to unpack"
---
() = (1, 2) ### "too many values to unpack"
---
# list assignment
load("asserts.star", "asserts")

[] = [] # empty ok

[a, b, c] = [1, 2, 3]
asserts.eq(a, 1)
asserts.eq(b, 2)
asserts.eq(c, 3)

[d, e, f,] = [1, 2, 3] # trailing comma ok
---
[a, b, c] = 1 ### "got int in sequence assignment"
---
[a, b] = [] ### "too few values to unpack"
---
[a, b] = [1] ### "too few values to unpack"
---
[a, b, c] = [1, 2] ### "too few values to unpack"
---
[a, b] = [1, 2, 3] ### "too many values to unpack"
---
[] = 1 ### "got int in sequence assignment"
---
[] = [1] ### "too many values to unpack"
---
[] = [1, 2] ### "too many values to unpack"
---
# list-tuple assignment
load("asserts.star", "asserts")

# empty ok
[] = ()
() = []

[a, b, c] = (1, 2, 3)
asserts.eq(a, 1)
asserts.eq(b, 2)
asserts.eq(c, 3)

[a2, b2, c2] = 1, 2, 3 # bare tuple ok

(d, e, f) = [1, 2, 3]
asserts.eq(d, 1)
asserts.eq(e, 2)
asserts.eq(f, 3)

[g, h, (i, j)] = (1, 2, [3, 4])
asserts.eq(g, 1)
asserts.eq(h, 2)
asserts.eq(i, 3)
asserts.eq(j, 4)

(k, l, [m, n]) = [1, 2, (3, 4)]
asserts.eq(k, 1)
asserts.eq(l, 2)
asserts.eq(m, 3)
asserts.eq(n, 4)

---
# option:nesteddef
# misc assignment
load("asserts.star", "asserts")

def assignment():
  a = [1, 2, 3]
  a[1] = 5
  asserts.eq(a, [1, 5, 3])
  a[-2] = 2
  asserts.eq(a, [1, 2, 3])
  asserts.eq("%d %d" % (5, 7), "5 7")
  x={}
  x[1] = 2
  x[1] += 3
  asserts.eq(x[1], 5)
  def f12(): x[(1, "abc", {})] = 1
  asserts.fails(f12, "unhashable type: dict")

assignment()

---
# augmented assignment

load("asserts.star", "asserts")

def f():
  x = 1
  x += 1
  asserts.eq(x, 2)
  x *= 3
  asserts.eq(x, 6)
f()

---
# effects of evaluating LHS occur only once

load("asserts.star", "asserts")

count = [0] # count[0] is the number of calls to f

def f():
  count[0] += 1
  return count[0]

x = [1, 2, 3]
x[f()] += 1

asserts.eq(x, [1, 3, 3]) # sole call to f returned 1
asserts.eq(count[0], 1) # f was called only once

---
# Order of evaluation.

load("asserts.star", "asserts")

calls = []

def f(name, result):
  calls.append(name)
  return result

# The right side is evaluated before the left in an ordinary assignment.
calls.clear()
f("array", [0])[f("index", 0)] = f("rhs", 0)
asserts.eq(calls, ["rhs", "array", "index"])

calls.clear()
f("lhs1", [0])[0], f("lhs2", [0])[0] = f("rhs1", 0), f("rhs2", 0)
asserts.eq(calls, ["rhs1", "rhs2", "lhs1", "lhs2"])

# Left side is evaluated first (and only once) in an augmented assignment.
calls.clear()
f("array", [0])[f("index", 0)] += f("addend", 1)
asserts.eq(calls, ["array", "index", "addend"])

---
# global referenced before assignment

def f():
   return g ### "global variable g referenced before assignment"

f()

g = 1

---
# option:nesteddef
# Free variables are captured by reference, so this is ok.
load("asserts.star", "asserts")

def f():
   def g():
     return outer
   outer = 1
   return g()

asserts.eq(f(), 1)

---
load("asserts.star", "asserts")

printok = [False]

# This program should resolve successfully but fail dynamically.
# However, the Java implementation currently reports the dynamic
# error at the x=1 statement (b/33975425).  I think we need to simplify
# the resolver algorithm to what we have implemented.
def use_before_def():
  print(x) # dynamic error: local var referenced before assignment
  printok[0] = True
  x = 1  # makes 'x' local

asserts.fails(use_before_def, 'local variable x referenced before assignment')
asserts.true(not printok[0]) # execution of print statement failed

---
x = [1]
x.extend([2]) # ok

def f():
   x += [4] ### "local variable x referenced before assignment"

f()

---

z += 3 ### "global variable z referenced before assignment"

---
load("asserts.star", "asserts")

# It's ok to define a global that shadows a built-in...
list = []
asserts.eq(type(list), "list")

# ...but then all uses refer to the global,
# even if they occur before the binding use.
# See github.com/google/skylark/issues/116.
asserts.fails(lambda: tuple, "global variable tuple referenced before assignment")
tuple = ()

---
# option:set
# Same as above, but set is dialect-specific;
# we shouldn't notice any difference.
load("asserts.star", "asserts")

set = [1, 2, 3]
asserts.eq(type(set), "list")

# As in Python 2 and Python 3,
# all 'in x' expressions in a comprehension are evaluated
# in the comprehension's lexical block, except the first,
# which is resolved in the outer block.
x = [[1, 2]]
asserts.eq([x for x in x for y in x],
          [[1, 2], [1, 2]])

---
# A comprehension establishes a single new lexical block,
# not one per 'for' clause.
x = [1, 2]
_ = [x for _ in [3] for x in x] ### "local variable x referenced before assignment"

---
load("asserts.star", "asserts")

# assign singleton sequence to 1-tuple
(x,) = (1,)
asserts.eq(x, 1)
(y,) = [1]
asserts.eq(y, 1)

# assign 1-tuple to variable
z = (1,)
asserts.eq(type(z), "tuple")
asserts.eq(len(z), 1)
asserts.eq(z[0], 1)

# assign value to parenthesized variable
(a) = 1
asserts.eq(a, 1)

---
# assignment to/from fields.
load("asserts.star", "asserts", "freeze")

hf = hasfields()
hf.x = 1
asserts.eq(hf.x, 1)
hf.x = [1, 2]
hf.x += [3, 4]
asserts.eq(hf.x, [1, 2, 3, 4])
freeze(hf)
def setX(hf):
  hf.x = 2
def setY(hf):
  hf.y = 3
asserts.fails(lambda: setX(hf), "cannot set field on a frozen hasfields")
asserts.fails(lambda: setY(hf), "cannot set field on a frozen hasfields")

---
# destucturing assignment in a for loop.
load("asserts.star", "asserts")

def f():
  res = []
  for (x, y), z in [(["a", "b"], 3), (["c", "d"], 4)]:
    res.append((x, y, z))
  return res
asserts.eq(f(), [("a", "b", 3), ("c", "d", 4)])

def g():
  a = {}
  for i, a[i] in [("one", 1), ("two", 2)]:
    pass
  return a
asserts.eq(g(), {"one": 1, "two": 2})

---
# parenthesized LHS in augmented assignment (success)
# option:globalreassign
load("asserts.star", "asserts")

a = 5
(a) += 3
asserts.eq(a, 8)

---
# parenthesized LHS in augmented assignment (error)

(a) += 5 ### "global variable a referenced before assignment"

---
# option:globalreassign
load("asserts.star", "asserts")
asserts = 1
load("asserts.star", "asserts")

---
# option:globalreassign option:loadbindsglobally
load("asserts.star", "asserts")
asserts = 1
load("asserts.star", "asserts")

---
# option:loadbindsglobally
_ = assert ### "global variable assert referenced before assignment"
load("asserts.star", "asserts")

---
_ = assert ### "local variable assert referenced before assignment"
load("asserts.star", "asserts")

---
def f(): asserts.eq(1, 1) # forward ref OK
load("asserts.star", "asserts")
f()

---
# option:loadbindsglobally
def f(): asserts.eq(1, 1) # forward ref OK
load("asserts.star", "asserts")
f()
