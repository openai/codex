# @generated
# To regenerate, run:
# ```
# STARLARK_RUST_REGENERATE_GOLDEN_TESTS=1 cargo test -p starlark --lib
# ```

# name

## MAGIC

```python
MAGIC: int
```

---

## Magic

```python
Magic: magic
```

---

## Obj

```python
Obj: obj
```

These are where the module docs go

---

## func1

```python
def func1(foo: str) -> str
```

Docs for func1

#### Parameters

* `foo`: Docs for foo


#### Returns

The string 'func1'

---

## func2

```python
def func2() -> str
```

---

## pos\_either\_named

```python
def pos_either_named(a: int, /, b: int, *, c: int) -> magic
```

---

## with\_defaults

```python
def with_defaults(
    explicit_default: list[str] = [],
    hidden_default: list[str] = ...,
    string_default: str = "my_default",
) -> None
```
