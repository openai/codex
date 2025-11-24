# @generated
# Copied from https://github.com/google/starlark-go/blob/e81fc95f7bd5bb1495fe69f27c1a99fcc77caa48/starlark/testdata/

# Tests of Starlark 'bool'

load("asserts.star", "asserts")

# truth
asserts.true(True)
asserts.true(not False)
asserts.true(not not True)
asserts.true(not not 1 >= 1)

# precedence of not
asserts.true(not not 2 > 1)
# asserts.true(not (not 2) > 1)   # TODO(adonovan): fix: gives error for False > 1.
# asserts.true(not ((not 2) > 1)) # TODO(adonovan): fix
# asserts.true(not ((not (not 2)) > 1)) # TODO(adonovan): fix
# asserts.true(not not not (2 > 1))

# bool conversion
asserts.eq(
    [bool(), bool(1), bool(0), bool("hello"), bool("")],
    [False, True, False, True, False],
)

# comparison
asserts.true(None == None)
asserts.true(None != False)
asserts.true(None != True)
asserts.eq(1 == 1, True)
asserts.eq(1 == 2, False)
asserts.true(False == False)
asserts.true(True == True)

# ordered comparison
asserts.true(False < True)
asserts.true(False <= True)
asserts.true(False <= False)
asserts.true(True > False)
asserts.true(True >= False)
asserts.true(True >= True)

# conditional expression
asserts.eq(1 if 3 > 2 else 0, 1)
asserts.eq(1 if "foo" else 0, 1)
asserts.eq(1 if "" else 0, 0)

# short-circuit evaluation of 'and' and 'or':
# 'or' yields the first true operand, or the last if all are false.
asserts.eq(0 or "" or [] or 0, 0)
asserts.eq(0 or "" or [] or 123 or 1 // 0, 123)
asserts.fails(lambda : 0 or "" or [] or 0 or 1 // 0, "division by zero")

# 'and' yields the first false operand, or the last if all are true.
asserts.eq(1 and "a" and [1] and 123, 123)
asserts.eq(1 and "a" and [1] and 0 and 1 // 0, 0)
asserts.fails(lambda : 1 and "a" and [1] and 123 and 1 // 0, "division by zero")

# Built-ins that want a bool want an actual bool, not a truth value.
# See github.com/bazelbuild/starlark/issues/30
asserts.eq(''.splitlines(True), [])
asserts.fails(lambda: ''.splitlines(1), 'got int, want bool')
asserts.fails(lambda: ''.splitlines("hello"), 'got string, want bool')
asserts.fails(lambda: ''.splitlines(0.0), 'got float, want bool')
