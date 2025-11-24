#![forbid(unsafe_code)]

/// Find the offset in bytes of the given `$field` of `$Type`. Requires an
/// already initialized `$instance` value to work with.
///
/// This is similar to the macro from [`memoffset`](https://docs.rs/memoffset),
/// however it uses no `unsafe` code.
///
/// This macro has a 3-argument and 2-argument version.
/// * In the 3-arg version you specify an instance of the type, the type itself,
///   and the field name.
/// * In the 2-arg version the macro will call the [`default`](Default::default)
///   method to make a temporary instance of the type for you.
///
/// The output of this macro is the byte offset of the field (as a `usize`). The
/// calculations of the macro are fixed across the entire program, but if the
/// type used is `repr(Rust)` then they're *not* fixed across compilations or
/// compilers.
///
/// ## Examples
///
/// ### 3-arg Usage
///
/// ```rust
/// # use bytemuck::offset_of;
/// // enums can't derive default, and for this example we don't pick one
/// enum MyExampleEnum {
///   A,
///   B,
///   C,
/// }
///
/// // so now our struct here doesn't have Default
/// #[repr(C)]
/// struct MyNotDefaultType {
///   pub counter: i32,
///   pub some_field: MyExampleEnum,
/// }
///
/// // but we provide an instance of the type and it's all good.
/// let val = MyNotDefaultType { counter: 5, some_field: MyExampleEnum::A };
/// assert_eq!(offset_of!(val, MyNotDefaultType, some_field), 4);
/// ```
///
/// ### 2-arg Usage
///
/// ```rust
/// # use bytemuck::offset_of;
/// #[derive(Default)]
/// #[repr(C)]
/// struct Vertex {
///   pub loc: [f32; 3],
///   pub color: [f32; 3],
/// }
/// // if the type impls Default the macro can make its own default instance.
/// assert_eq!(offset_of!(Vertex, loc), 0);
/// assert_eq!(offset_of!(Vertex, color), 12);
/// ```
///
/// # Usage with `#[repr(packed)]` structs
///
/// Attempting to compute the offset of a `#[repr(packed)]` struct with
/// `bytemuck::offset_of!` requires an `unsafe` block. We hope to relax this in
/// the future, but currently it is required to work around a soundness hole in
/// Rust (See [rust-lang/rust#27060]).
///
/// [rust-lang/rust#27060]: https://github.com/rust-lang/rust/issues/27060
///
/// <p style="background:rgba(255,181,77,0.16);padding:0.75em;">
/// <strong>Warning:</strong> This is only true for versions of bytemuck >
/// 1.4.0. Previous versions of
/// <code style="background:rgba(41,24,0,0.1);">bytemuck::offset_of!</code>
/// will only emit a warning when used on the field of a packed struct in safe
/// code, which can lead to unsoundness.
/// </p>
///
/// For example, the following will fail to compile:
///
/// ```compile_fail
/// #[repr(C, packed)]
/// #[derive(Default)]
/// struct Example {
///   field: u32,
/// }
/// // Doesn't compile:
/// let _offset = bytemuck::offset_of!(Example, field);
/// ```
///
/// While the error message this generates will mention the
/// `safe_packed_borrows` lint, the macro will still fail to compile even if
/// that lint is `#[allow]`ed:
///
/// ```compile_fail
/// # #[repr(C, packed)] #[derive(Default)] struct Example { field: u32 }
/// // Still doesn't compile:
/// #[allow(safe_packed_borrows)]
/// {
///   let _offset = bytemuck::offset_of!(Example, field);
/// }
/// ```
///
/// This *can* be worked around by using `unsafe`, but it is only sound to do so
/// if you can guarantee that taking a reference to the field is sound.
///
/// In practice, this means it only works for fields of align(1) types, or if
/// you know the field's offset in advance (defeating the point of `offset_of`)
/// and can prove that the struct's alignment and the field's offset are enough
/// to prove the field's alignment.
///
/// Once the `raw_ref` macros are available, a future version of this crate will
/// use them to lift the limitations of packed structs. For the duration of the
/// `1.x` version of this crate that will be behind an on-by-default cargo
/// feature (to maintain minimum rust version support).
#[macro_export]
macro_rules! offset_of {
  ($instance:expr, $Type:path, $field:tt) => {{
    #[forbid(safe_packed_borrows)]
    {
      // This helps us guard against field access going through a Deref impl.
      #[allow(clippy::unneeded_field_pattern)]
      let $Type { $field: _, .. };
      let reference: &$Type = &$instance;
      let address = reference as *const _ as usize;
      let field_pointer = &reference.$field as *const _ as usize;
      // These asserts/unwraps are compiled away at release, and defend against
      // the case where somehow a deref impl is still invoked.
      let result = field_pointer.checked_sub(address).unwrap();
      assert!(result <= $crate::__core::mem::size_of::<$Type>());
      result
    }
  }};
  ($Type:path, $field:tt) => {{
    $crate::offset_of!(<$Type as Default>::default(), $Type, $field)
  }};
}
