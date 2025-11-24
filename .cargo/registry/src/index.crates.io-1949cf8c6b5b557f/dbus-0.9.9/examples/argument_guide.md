Preamble
--------

The different ways you can append and get message arguments can be a bit bewildering. I've iterated a few times on the design and didn't want to lose backwards compatibility.

This guide is to help you on your way. In addition, many of the examples in the examples directory append and read arguments.
There is also a reference at the end of this document.

Code generation
---------------

First - if you can get D-Bus introspection data, you can use the the `dbus-codegen` tool to generate some boilerplate code for you. E g, if you want to talk to NetworkManager:

```rust
cargo install dbus-codegen
dbus-codegen-rust -s -g -m None -d org.freedesktop.NetworkManager -p /org/freedesktop/NetworkManager > networkmanager.rs
```

You would then use this code like:

```rust
// main.rs
mod networkmanager;

/* ... */

// Start a connection to the system bus.
let c = Connection::new_system()?;

// Make a "ConnPath" struct that just contains a Connection, a destination and a path.
let p = c.with_proxy("org.freedesktop.NetworkManager", "/org/freedesktop/NetworkManager", Duration::new(5, 0));

// Bring our generated code into scope.
use networkmanager::OrgFreedesktopNetworkManager;

// Now we can call methods on our connpath from the "org.freedesktop.NetworkManager" interface.
let devices = p.get_all_devices()?;
```

There is also pre-generated code for standard D-Bus interfaces in the `stdintf` module. A similar example:

```rust
let c = Connection::new_session()?;

// Make a "ConnPath" struct that just contains a Connection, a destination and a path.
let p = c.with_path("org.mpris.MediaPlayer2.rhythmbox", "/org/mpris/MediaPlayer2", Duration::new(5, 0));

// The ConnPath struct implements many traits, e g `org.freedesktop.DBus.Properties`. Bring the trait into scope.
use stdintf::org_freedesktop_dbus::Properties;

// Now we can call org.freedesktop.DBus.Properties.Get just like an ordinary method and get the result back.
let metadata = p.get("org.mpris.MediaPlayer2.Player", "Metadata")?;
```

For more details, see `dbus-codegen-rust --help` and the `README.md` in the dbus-codegen directory.

Now, if you want to make a service yourself, the generated code is more complex. And for some use cases, codegen isn't really an option, so let's move on:

Append / get basic types
------------------------

If you just want to get/append simple types, just use `append1` / `append2` / `append3`, and
`read1` / `read2` / `read3`. The imaginary method below takes one byte parameter and one string parameter, and returns one string parameter and one int parameter.

```rust
let m = Message::new_method_call(dest, path, intf, member)?.append2(5u8, "Foo");
let r = c.send_with_reply_and_block(m, 2000)?;
let (data1, data2): (&str, i32) = c.read2()?;
```

Arrays and dictionaries
-----------------------

D-Bus arrays and dictionaries usually correspond to `Vec` and `HashMap`. You can just append and get them like basic types:

```rust
let v = vec![3i32, 4i32, 5i32];
let mut map = HashMap::new();
map.insert("Funghi", 5u16);
map.insert("Mold", 8u16);

let m = Message::new_method_call(dest, path, intf, member)?.append2(v, map);
let r = c.send_with_reply_and_block(m, 2000)?;
let (data1, data2): (Vec<i32>, HashMap<&str, u16>) = r.read2()?;
```

Or combine them as you wish, e g, use a `Vec<Vec<u8>>`, a `HashMap<u64, Vec<String>>` or `HashMap<String, HashMap<String, i32>>` to construct more difficult types.

Slices can sometimes be used as arrays - e g, `&[&str]` can be appended, but only very simple types can be used with `get` and `read`, e g `&[u8]`.

This is the easiest way to get started, but in case you want to avoid the overhead of creating `Vec` or `HashMap`s, the "Array and Dict types" and "Iter / IterAppend" sections offer useful alternatives.

Variants
--------

Things are getting slightly more complex with Variants, because they are not strongly typed and thus not fit as well into Rust's strongly typed as arrays and dicts.

If you know the type beforehand, it's still easy:

```rust
let v = Variant("This is a variant containing a &str");
let m = Message::new_method_call(dest, path, intf, member)?.append1(v);
let r = c.send_with_reply_and_block(m, 2000)?;
let z: Variant<i32> = r.read1()?;
println!("Method returned {}", z.0);
```

The `Variant` struct is just a wrapper with a public interior, so you can easily both read from it and write to it with the `.0` accessor.

Sometimes you don't know the type beforehand. We can solve this in two ways (choose whichever is more appropriate for your use case), either through the trait object `Box<dyn RefArg>` or through `Iter` / `IterAppend` (see later sections).

Through trait objects:

```rust
let x = Box::new(5000i32) as Box<dyn RefArg>;
let m = Message::new_method_call(dest, path, intf, member)?.append1(Variant(x));
let r = c.send_with_reply_and_block(m, 2000)?;
let z: Variant<Box<dyn RefArg>> = r.read1()?;
```

Ok, so we retrieved our `Box<dyn RefArg>`. We now need to use the `RefArg` methods to probe it, to see what's inside.
You can use `as_i64`, `as_u64`, `as_f64` or `as_str` if you want to test for number or string types.
Second easiest is to use `arg::cast` to downcast to the specific type inside (see reference section below
for what type you need to cast to).

This works for most types, but in some advanced cases, this is not possible, either because the `RefArg` is not `'static`
(work around this with the `box_clone` method if necessary), or because the internal representation is not specified.
In this case, try to use `as_static_inner` or `as_iter` to iterate through the interior of the complex type.

```rust
let z: Variant<Box<dyn RefArg + 'static>> = r.read1()?;
let value = &z.0;

if let Some(s) = value.as_str() { println!("It's a string: {}", s); }
else if let Some(i) = value.as_i64() { println!("It's an integer: {}", i); }
else if let Some(f) = arg::cast::<f64>(value) { println!("It's a float: {}", f); }
else { println!("Don't know how to handle a {:?}", value.arg_type()) }
```

Dicts and variants are sometimes combined, e g, you might need to read a D-Bus dictionary of String to Variants.
You can read these as `PropMap` (which is a type alias for `HashMap<String, Variant<Box<dyn RefArg>>>`) and use
`prop_cast` to retrieve a value.

Structs
-------

D-Bus structs are implemented as Rust tuples. You can append and get tuples like you do with other types of arguments.
You can also use `VecDeque<Box<dyn RefArg>>` for when the types of the struct cannot be statically typed.

For an example of the latter, the [Desktop Notifications Specification](specifications.freedesktop.org/notification-spec/latest/index.html) specifies that the `notify()` method sends a dictionary of optional [hints](https://specifications.freedesktop.org/notification-spec/latest/hints.html), one of which is `"image-data"` hint, which contains a struct.  Here's how you might retrieve that `"image-data"` hint and cast it into a rust struct.

Implementing the `notify()` method in Rust -- trait generated by [`dbus-codegen-rust`](https://github.com/diwic/dbus-rs/tree/master/dbus-codegen):
```rust
// impl OrgFreedesktopNotifications {
fn notify(
    &mut self,
    app_name: String,
    replaces_id: u32,
    app_icon: String,
    summary: String,
    body: String,
    actions: Vec<String>,
    hints: arg::PropMap,
    expire_timeout: i32
) -> Result<u32, dbus::MethodErr> {
    use dbus::arg::prop_cast;

    // Retrieving a property from a PropMap of type byte (`u8`).
    let urgency: Option<&u8> = prop_cast(&hints, "urgency");

    // Retrieving a property from a PropMap of type struct (`Image`, in this case).
    struct Image {
        width: i32,
        height: i32,
        rowstride: i32,
        one_point_two_bit_alpha: bool,
        bits_per_sample: i32,
        channels: i32,
        data: Vec<u8>,
    }

    // Structs are represented internally as `VecDeque<Box<RefArg>>`.
    let image_data: Option<&VecDeque<Box<dyn RefArg>>> = prop_cast(&hints, "image-data");
    let image = if let Some(img) = image_data {
        use dbus::arg::cast;
        // NOTE: It's possible that someone sent the "image-data" hint but didn't fill it correctly, which will
        // panic with this code!  You should make sure that the length of the `Vec` is correct and handle the
        // `cast` `Option` instead of just unwrapping.
        let width = *cast::<i32>(&img[0]).unwrap();
        let height = *cast::<i32>(&img[1]).unwrap();
        let rowstride = *cast::<i32>(&img[2]).unwrap();
        let one_point_two_bit_alpha = *cast::<bool>(&img[3]).unwrap();
        let bits_per_sample = *cast::<i32>(&img[4]).unwrap();
        let channels = *cast::<i32>(&img[5]).unwrap();
        let data = cast::<Vec<u8>>(&img[6]).unwrap().clone();
        Some(Image { width, height, rowstride, one_point_two_bit_alpha, bits_per_sample, channels, data })
    } else {
        None
    }

    // Do something with image / urgency.

    Ok(0)
}
```

TODO: tuple example

Declare method arguments
------------------------

For `dbus-crossroads`, the method argument types are automatically deduced from your method closure, but the names have to repeated. Method arguments - both in and out - are always tuples, so if you have a single argument, it needs to be wrapped into a "one element tuple" (so a variable or type `x` becomes `(x,)`). Like this:

```rust
b.method("Hello", ("request",), ("reply",), |_, _, (request,): (HashMap<i32, Vec<(i32, bool, String)>>,)| {
  // Returns a Result<String>
});
```

For `dbus-tree`, you want to declare what input and output arguments your method expects - so that correct D-Bus introspection data can be generated. You'll use the same types as you learned earlier in this guide:

```rust
factory.method( /* ... */ )
.inarg::<HashMap<i32, Vec<(i32, bool, String)>>,_>("request")
.outarg::<&str,_>("reply")
```

The types are just for generating a correct signature, they are never instantiated. Many different types can generate the same signature - e g, `Array<u8, _>`, `Vec<u8>` and `&[u8]` will all generate the same signature. `Variant` will generate the same type signature regardless of what's inside, so just write `Variant<()>` for simplicity.

Iter / IterAppend
-----------------

Iter and IterAppend are more low-level, direct methods to get and append arguments. They can, e g, come handy if you have more than five arguments to read.

E g, for appending a variant with IterAppend you can use `IterAppend::new(&msg).append_variant(|i| i.append(5000i32))` to append what you need to your variant inside the closure.
To read a variant you can use `let i = msg.read1::<Variant<Iter>>::()?` and then examine the methods on `i.0` to probe the variant.

Array and Dict types
--------------------

These provide slightly better flexibility than using `Vec` and `HashMap` by instead integrating with `Iterator`. Here's an example where you can append and get a dictionary without having to create a HashMap:

```rust
let x = &[("Hello", true), ("World", false)];
let m = Message::new_method_call(dest, path, intf, member)?.append1(Dict::new(x));
let r = c.send_with_reply_and_block(m, 2000)?;
let z: Dict<i32, &str, _> = r.read1()?;
for (key, value) in z { /* do something */ }
```

An edge case where this is necessary is having floating point keys in a dictionary - this is supported in D-Bus but not in Rust's `HashMap`. I have never seen this in practice, though.

Unusual types
-------------

The types `Path` and `Signature` are not often used, but they can be appended and read as other argument types. `Path` and `Signature` will return strings with a borrowed lifetime - use `.into_static()` if you want to untie that lifetime.

You can also append and get a `std::fs::File`, this will send or receive a file descriptor. `OwnedFd` is an earlier design which was used for the same thing.

MessageItem
-----------

MessageItem was the first design - an enum representing a D-Bus argument. It still works, but I doubt you'll ever need to use it. Newer methods provide better type safety, speed, and ergonomics.

Reference
=========

This is a translation table between D-Bus types and Rust types. Both the RefArg type
and the other types can be used in most cases, e g `append`, `get` and/or `read`, as
well as function arguments for `dbus-crossroads`. In addition, `&T`, `Box<T>`, `Rc<T>` and
`Arc<T>` can (in general) be used instead of `T`, where `T` is a Rust type.

If you want to to use `cast` or `prop_cast` on a `&RefArg` however, you need to use
the RefArg type. If the RefArg type is listed as N/A, it means that there is no stable RefArg type
and casting is not supported.

| D-Bus type | Signature | RefArg type | Other types |
| ---------- | --------- | ----------- | ----------- |
| BYTE | `y` | `u8` | |
| BOOLEAN | `b` | `bool` | |
| INT16	| `n` | `i16` | |
| UINT16 | `q` | `u16` | |
| INT32 | `i` | `i32` | |
| UINT32 | `u` | `u32` | |
| INT64 | `x` | `i64` | |
| UINT64 | `t` | `u64` | |
| DOUBLE | `d` | `f64` | |
| STRING | `s` | `String` | `&str`, `&CStr` |
| UNIX_FD | `h` | `File` | `OwnedFd` |
| OBJECT_PATH | `o` | `Path<'static>` | `Path<'a>` |
| SIGNATURE	| `g` | `Signature<'static>` | `Signature<'a>` |
| VARIANT | `v` | `Variant<Box<RefArg>>` | `Variant<T>`, `Variant<Iter>` |
| STRUCT | `(`...`)` | `VecDeque<Box<RefArg>>` | tuples: `(T,)`, `(T1,T2)` etc |
| DICT<STRING, VARIANT> | `a{sv}` | `PropMap` | |
| DICT<_, _> | `a{`...`}` | N/A | `HashMap<K, V>`, `Dict<K, V, _>`, `BTreeMap<K, V>` |
| ARRAY<ARRAY<_>> | `aa`... | N/A | `Vec<Vec<T>>`, `&[&[T]]`, `Array<Array<T>>` |
| ARRAY<DICT<_>> | `aa{`...`}` | N/A | `Vec<HashMap<K, V>>`, `&[Dict<K, V, _>]` |
| ARRAY<STRUCT<_>> | `a(`...`)` | N/A | `Vec<VecDeque<Box<RefArg>>>`, `Vec<`tuple`>` |
| ARRAY<_> (all else) | `a`... | `Vec<T>` | `&[T]` `Array<T, _>` |
