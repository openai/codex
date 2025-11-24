# Notes on porting to zbus

Plan: start a section of code that will be easy convert to zbus, while keeping dbus dependency. I think that this would be `Item`, which is a "leaf" struct (meaning it doesn't create other dbus-connected structs). This way I only have to worry about any data being received, and not so much data being returned.

Plan, revised: I should still start with Item. However, the place where I'll slide zbus in is a little different then I had thought. This is because my code currently is structured:
- With an `Interface` (which should actually be called a `proxy` that talks to the interface) that is a very low-level implementation of basics like method calls.
- And then all the details of talking to the service interface are handled directly in `Item`'s methods.

With zbus, I would instead have:
- A derived `dbus_proxy`, which would set up all the boilerplate for talking to a an interface, and then
- Methods in `Item` would generally just be calling the derived `dbus_proxy`.
- This means that with zbus, I should be able to get rid of `Interface`.

So, my first step should be to replace usage of the `Item` proxy.

## First Pass
Creating proxy was nice, and removed a lot of my boilerplate.

One issue was lifetimes for using a path in creating a new proxy. I had wanted to store the proxy in the `Item` struct, but I needed to borrow the `item_path`, which wouldn't live long enough. I ended up instantiating a new proxy on each `Item` method, which didn't feel as good.

Now, getting some test errors. One is that session doesn't exist.

## Second Pass
First finished creating all the proxies.

I found `Proxy::new_for_owned` which removed the instantiation of a proxy per-method, now the `Proxy` can be saved in the struct.

There's a weird issue where on a derived `property` it allows `ObjectPath` in the return, but on methods it requires `OwnedObjectPath`. Seems to work ok, oh well.

Looks like `SecretStruct` needs to be wrapped in another struct in order to fit the dbus signature, not sure what that's about.

Otherwise pretty straightforward. Removing the low-level details of creating dbus types allowed me to refactor much more easily.
