## Rust for Windows

The [windows](https://crates.io/crates/windows) and [windows-sys](https://crates.io/crates/windows-sys) crates let you call any Windows API past, present, and future using code generated on the fly directly from the [metadata describing the API](https://github.com/microsoft/windows-rs/tree/master/crates/libs/bindgen/default) and right into your Rust package where you can call them as if they were just another Rust module. The Rust language projection follows in the tradition established by [C++/WinRT](https://github.com/microsoft/cppwinrt) of building language projections for Windows using standard languages and compilers, providing a natural and idiomatic way for Rust developers to call Windows APIs.

* [Getting started](https://kennykerr.ca/rust-getting-started/)
* [Samples](https://github.com/microsoft/windows-rs/tree/0.52.0/crates/samples) <!-- link to samples for upcoming release -->
* [Releases](https://github.com/microsoft/windows-rs/releases)

Start by adding the following to your Cargo.toml file:

```toml
[dependencies.windows]
version = "0.52"
features = [
    "Data_Xml_Dom",
    "Win32_Foundation",
    "Win32_Security",
    "Win32_System_Threading",
    "Win32_UI_WindowsAndMessaging",
]
```

Make use of any Windows APIs as needed:

```rust,no_run
use windows::{
    core::*, Data::Xml::Dom::*, Win32::Foundation::*, Win32::System::Threading::*,
    Win32::UI::WindowsAndMessaging::*,
};

fn main() -> Result<()> {
    let doc = XmlDocument::new()?;
    doc.LoadXml(h!("<html>hello world</html>"))?;

    let root = doc.DocumentElement()?;
    assert!(root.NodeName()? == "html");
    assert!(root.InnerText()? == "hello world");

    unsafe {
        let event = CreateEventW(None, true, false, None)?;
        SetEvent(event)?;
        WaitForSingleObject(event, 0);
        CloseHandle(event)?;

        MessageBoxA(None, s!("Ansi"), s!("Caption"), MB_OK);
        MessageBoxW(None, w!("Wide"), w!("Caption"), MB_OK);
    }

    Ok(())
}
```

## windows-sys

The `windows-sys` crate is a zero-overhead fallback for the most demanding situations and primarily where the absolute best compile time is essential. It only includes function declarations (externs), structs, and constants. No convenience helpers, traits, or wrappers are provided.

Start by adding the following to your Cargo.toml file:

```toml
[dependencies.windows-sys]
version = "0.52"
features = [
    "Win32_Foundation",
    "Win32_Security",
    "Win32_System_Threading",
    "Win32_UI_WindowsAndMessaging",
]
```

Make use of any Windows APIs as needed:

```rust,no_run
use windows_sys::{
    core::*, Win32::Foundation::*, Win32::System::Threading::*, Win32::UI::WindowsAndMessaging::*,
};

fn main() {
    unsafe {
        let event = CreateEventW(std::ptr::null(), 1, 0, std::ptr::null());
        SetEvent(event);
        WaitForSingleObject(event, 0);
        CloseHandle(event);

        MessageBoxA(0, s!("Ansi"), s!("Caption"), MB_OK);
        MessageBoxW(0, w!("Wide"), w!("Caption"), MB_OK);
    }
}
```
