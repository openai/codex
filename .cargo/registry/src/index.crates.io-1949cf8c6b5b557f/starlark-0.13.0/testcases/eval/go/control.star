# @generated
# Copied from https://github.com/google/starlark-go/blob/e81fc95f7bd5bb1495fe69f27c1a99fcc77caa48/starlark/testdata/

# Tests of Starlark control flow

load("asserts.star", "asserts")

def controlflow():
  # elif
  x = 0
  if True:
    x=1
  elif False:
    asserts.fail("else of true")
  else:
    asserts.fail("else of else of true")
  asserts.true(x)

  x = 0
  if False:
    asserts.fail("then of false")
  elif True:
    x = 1
  else:
    asserts.fail("else of true")
  asserts.true(x)

  x = 0
  if False:
    asserts.fail("then of false")
  elif False:
    asserts.fail("then of false")
  else:
    x = 1
  asserts.true(x)
controlflow()

def loops():
  y = ""
  for x in [1, 2, 3, 4, 5]:
    if x == 2:
      continue
    if x == 4:
      break
    y = y + str(x)
  return y
asserts.eq(loops(), "13")

# return
g = 123
def f(x):
  for g in (1, 2, 3):
    if g == x:
      return g
asserts.eq(f(2), 2)
asserts.eq(f(4), None) # falling off end => return None
asserts.eq(g, 123) # unchanged by local use of g in function

# infinite sequences
def fib(n):
  seq = []
  for x in fibonacci: # fibonacci is an infinite iterable defined in eval_test.go
    if len(seq) == n:
      break
    seq.append(x)
  return seq
asserts.eq(fib(10),  [0, 1, 1, 2, 3, 5, 8, 13, 21, 34])
