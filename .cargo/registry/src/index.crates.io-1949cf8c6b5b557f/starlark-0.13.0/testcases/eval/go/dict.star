# @generated
# Copied from https://github.com/google/starlark-go/blob/e81fc95f7bd5bb1495fe69f27c1a99fcc77caa48/starlark/testdata/

# Tests of Starlark 'dict'
# option:nesteddef

load("asserts.star", "asserts", "freeze")

# literals
asserts.eq({}, {})
asserts.eq({"a": 1}, {"a": 1})
asserts.eq({"a": 1,}, {"a": 1})

# truth
asserts.true({False: False})
asserts.true(not {})

# dict + dict is no longer supported.
asserts.fails(lambda: {"a": 1} + {"b": 2}, 'unknown binary op: dict \\+ dict')

# dict comprehension
asserts.eq({x: x*x for x in range(3)}, {0: 0, 1: 1, 2: 4})

# dict.pop
x6 = {"a": 1, "b": 2}
asserts.eq(x6.pop("a"), 1)
asserts.eq(str(x6), '{"b": 2}')
asserts.fails(lambda: x6.pop("c"), "pop: missing key")
asserts.eq(x6.pop("c", 3), 3)
asserts.eq(x6.pop("c", None), None) # default=None tests an edge case of UnpackArgs
asserts.eq(x6.pop("b"), 2)
asserts.eq(len(x6), 0)

# dict.popitem
x7 = {"a": 1, "b": 2}
asserts.eq([x7.popitem(), x7.popitem()], [("a", 1), ("b", 2)])
asserts.fails(x7.popitem, "empty dict")
asserts.eq(len(x7), 0)

# dict.keys, dict.values
x8 = {"a": 1, "b": 2}
asserts.eq(x8.keys(), ["a", "b"])
asserts.eq(x8.values(), [1, 2])

# equality
asserts.eq({"a": 1, "b": 2}, {"a": 1, "b": 2})
asserts.eq({"a": 1, "b": 2,}, {"a": 1, "b": 2})
asserts.eq({"a": 1, "b": 2}, {"b": 2, "a": 1})

# insertion order is preserved
asserts.eq(dict([("a", 0), ("b", 1), ("c", 2), ("b", 3)]).keys(), ["a", "b", "c"])
asserts.eq(dict([("b", 0), ("a", 1), ("b", 2), ("c", 3)]).keys(), ["b", "a", "c"])
asserts.eq(dict([("b", 0), ("a", 1), ("b", 2), ("c", 3)])["b"], 2)
# ...even after rehashing (which currently occurs after key 'i'):
small = dict([("a", 0), ("b", 1), ("c", 2)])
small.update([("d", 4), ("e", 5), ("f", 6), ("g", 7), ("h", 8), ("i", 9), ("j", 10), ("k", 11)])
asserts.eq(small.keys(), ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k"])

# Duplicate keys are not permitted in dictionary expressions (see b/35698444).
# (Nor in keyword args to function calls---checked by resolver.)
asserts.fails(lambda: {"aa": 1, "bb": 2, "cc": 3, "bb": 4}, 'duplicate key: "bb"')

# Check that even with many positional args, keyword collisions are detected.
asserts.fails(lambda: dict({'b': 3}, a=4, **dict(a=5)), 'dict: duplicate keyword arg: "a"')
asserts.fails(lambda: dict({'a': 2, 'b': 3}, a=4, **dict(a=5)), 'dict: duplicate keyword arg: "a"')
# positional/keyword arg key collisions are ok
asserts.eq(dict((['a', 2], ), a=4), {'a': 4})
asserts.eq(dict((['a', 2], ['a', 3]), a=4), {'a': 4})

# index
def setIndex(d, k, v):
  d[k] = v

x9 = {}
asserts.fails(lambda: x9["a"], 'key "a" not in dict')
x9["a"] = 1
asserts.eq(x9["a"], 1)
asserts.eq(x9, {"a": 1})
asserts.fails(lambda: setIndex(x9, [], 2), 'unhashable type: list')
freeze(x9)
asserts.fails(lambda: setIndex(x9, "a", 3), 'cannot insert into frozen hash table')

x9a = {}
x9a[1, 2] = 3  # unparenthesized tuple is allowed here
asserts.eq(x9a.keys()[0], (1, 2))

# dict.get
x10 = {"a": 1}
asserts.eq(x10.get("a"), 1)
asserts.eq(x10.get("b"), None)
asserts.eq(x10.get("a", 2), 1)
asserts.eq(x10.get("b", 2), 2)

# dict.clear
x11 = {"a": 1}
asserts.contains(x11, "a")
asserts.eq(x11["a"], 1)
x11.clear()
asserts.fails(lambda: x11["a"], 'key "a" not in dict')
asserts.true("a" not in x11)
freeze(x11)
asserts.fails(x11.clear, "cannot clear frozen hash table")

# dict.setdefault
x12 = {"a": 1}
asserts.eq(x12.setdefault("a"), 1)
asserts.eq(x12["a"], 1)
asserts.eq(x12.setdefault("b"), None)
asserts.eq(x12["b"], None)
asserts.eq(x12.setdefault("c", 2), 2)
asserts.eq(x12["c"], 2)
asserts.eq(x12.setdefault("c", 3), 2)
asserts.eq(x12["c"], 2)
freeze(x12)
asserts.eq(x12.setdefault("a", 1), 1) # no change, no error
asserts.fails(lambda: x12.setdefault("d", 1), "cannot insert into frozen hash table")

# dict.update
x13 = {"a": 1}
x13.update(a=2, b=3)
asserts.eq(x13, {"a": 2, "b": 3})
x13.update([("b", 4), ("c", 5)])
asserts.eq(x13, {"a": 2, "b": 4, "c": 5})
x13.update({"c": 6, "d": 7})
asserts.eq(x13, {"a": 2, "b": 4, "c": 6, "d": 7})
freeze(x13)
asserts.fails(lambda: x13.update({"a": 8}), "cannot insert into frozen hash table")

# dict as a sequence
#
# for loop
x14 = {1:2, 3:4}
def keys(dict):
  keys = []
  for k in dict: keys.append(k)
  return keys
asserts.eq(keys(x14), [1, 3])
#
# comprehension
asserts.eq([x for x in x14], [1, 3])
#
# varargs
def varargs(*args): return args
x15 = {"one": 1}
asserts.eq(varargs(*x15), ("one",))

# kwargs parameter does not alias the **kwargs dict
def kwargs(**kwargs): return kwargs
x16 = kwargs(**x15)
asserts.eq(x16, x15)
x15["two"] = 2 # mutate
asserts.ne(x16, x15)

# iterator invalidation
def iterator1():
  dict = {1:1, 2:1}
  for k in dict:
    dict[2*k] = dict[k]
asserts.fails(iterator1, "insert.*during iteration")

def iterator2():
  dict = {1:1, 2:1}
  for k in dict:
    dict.pop(k)
asserts.fails(iterator2, "delete.*during iteration")

def iterator3():
  def f(d):
    d[3] = 3
  dict = {1:1, 2:1}
  _ = [f(dict) for x in dict]
asserts.fails(iterator3, "insert.*during iteration")

# This assignment is not a modification-during-iteration:
# the sequence x should be completely iterated before
# the assignment occurs.
def f():
  x = {1:2, 2:4}
  a, x[0] = x
  asserts.eq(a, 1)
  asserts.eq(x, {1: 2, 2: 4, 0: 2})
f()

# Regression test for a bug in hashtable.delete
def test_delete():
  d = {}

  # delete tail first
  d["one"] = 1
  d["two"] = 2
  asserts.eq(str(d), '{"one": 1, "two": 2}')
  d.pop("two")
  asserts.eq(str(d), '{"one": 1}')
  d.pop("one")
  asserts.eq(str(d), '{}')

  # delete head first
  d["one"] = 1
  d["two"] = 2
  asserts.eq(str(d), '{"one": 1, "two": 2}')
  d.pop("one")
  asserts.eq(str(d), '{"two": 2}')
  d.pop("two")
  asserts.eq(str(d), '{}')

  # delete middle
  d["one"] = 1
  d["two"] = 2
  d["three"] = 3
  asserts.eq(str(d), '{"one": 1, "two": 2, "three": 3}')
  d.pop("two")
  asserts.eq(str(d), '{"one": 1, "three": 3}')
  d.pop("three")
  asserts.eq(str(d), '{"one": 1}')
  d.pop("one")
  asserts.eq(str(d), '{}')

test_delete()

# Regression test for github.com/google/starlark-go/issues/128.
asserts.fails(lambda: dict(None), 'got NoneType, want iterable')
asserts.fails(lambda: {}.update(None), 'got NoneType, want iterable')

---
# Verify position of an "unhashable key" error in a dict literal.

_ = {
    "one": 1,
    ["two"]: 2, ### "unhashable type: list"
    "three": 3,
}

---
# Verify position of a "duplicate key" error in a dict literal.

_ = {
    "one": 1,
    "one": 1, ### `duplicate key: "one"`
    "three": 3,
}

---
# Verify position of an "unhashable key" error in a dict comprehension.

_ = {
    k: v ### "unhashable type: list"
    for k, v in [
        ("one", 1),
        (["two"], 2),
        ("three", 3),
    ]
}
