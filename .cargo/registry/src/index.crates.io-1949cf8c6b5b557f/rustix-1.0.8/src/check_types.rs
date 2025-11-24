//! Macros for checking that types have the same layout as other types.

#![allow(unused_macros)]

/// Check that the size and alignment of a type match the `sys` bindings.
macro_rules! check_type {
    ($struct:ident) => {
        assert_eq_size!($struct, c::$struct);
        assert_eq_align!($struct, c::$struct);
    };
}

/// The same as `check_type`, but for unions and anonymous structs we've
/// renamed to avoid having types like `bindgen_ty_1` in the API.
macro_rules! check_renamed_type {
    ($to:ident, $from:ident) => {
        assert_eq_size!($to, c::$from);
        assert_eq_align!($to, c::$from);
    };
}

/// Check that the field of a struct has the same offset as the corresponding
/// field in the `sys` bindings.
macro_rules! check_struct_field {
    ($struct:ident, $field:ident) => {
        const_assert_eq!(
            memoffset::offset_of!($struct, $field),
            memoffset::offset_of!(c::$struct, $field)
        );

        // This can't use `const_assert_eq` because `span_of` returns a
        // `Range`, which can't be compared in const contexts.
        assert_eq!(
            memoffset::span_of!($struct, $field),
            memoffset::span_of!(c::$struct, $field)
        );
    };
}

/// The same as `check_struct_field`, but for unions and anonymous structs
/// we've renamed to avoid having types like `bindgen_ty_1` in the API.
macro_rules! check_struct_renamed_field {
    ($struct:ident, $to:ident, $from:ident) => {
        const_assert_eq!(
            memoffset::offset_of!($struct, $to),
            memoffset::offset_of!(c::$struct, $from)
        );

        // As above, this can't use `const_assert_eq`.
        assert_eq!(
            memoffset::span_of!($struct, $to),
            memoffset::span_of!(c::$struct, $from)
        );
    };
}

/// The same as `check_struct_field`, but for when the struct is renamed
/// but the field is not.
macro_rules! check_renamed_struct_field {
    ($to_struct:ident, $from_struct:ident, $field:ident) => {
        const_assert_eq!(
            memoffset::offset_of!($to_struct, $field),
            memoffset::offset_of!(c::$from_struct, $field)
        );

        // As above, this can't use `const_assert_eq`.
        assert_eq!(
            memoffset::span_of!($to_struct, $field),
            memoffset::span_of!(c::$from_struct, $field)
        );
    };
}

/// The same as `check_struct_renamed_field`, but for when both the struct and
/// a field are renamed.
macro_rules! check_renamed_struct_renamed_field {
    ($to_struct:ident, $from_struct:ident, $to:ident, $from:ident) => {
        const_assert_eq!(
            memoffset::offset_of!($to_struct, $to),
            memoffset::offset_of!(c::$from_struct, $from)
        );

        // As above, this can't use `const_assert_eq`.
        assert_eq!(
            memoffset::span_of!($to_struct, $to),
            memoffset::span_of!(c::$from_struct, $from)
        );
    };
}

/// For the common case of no renaming, check all fields of a struct.
macro_rules! check_struct {
    ($name:ident, $($field:ident),*) => {
        // Check the size and alignment.
        check_type!($name);

        // Check that we have all the fields.
        if false {
            #[allow(unreachable_code)]
            let _test = $name {
                $($field: panic!()),*
            };
            #[allow(unreachable_code)]
            let _test = c::$name {
                $($field: panic!()),*
            };
        }

        // Check that the fields have the right sizes and offsets.
        $(check_struct_field!($name, $field));*
    };
}

/// For the case of renaming, check all fields of a struct.
macro_rules! check_renamed_struct {
    ($to_struct:ident, $from_struct:ident, $($field:ident),*) => {
        // Check the size and alignment.
        check_renamed_type!($to_struct, $from_struct);

        // Check that we have all the fields.
        if false {
            #[allow(unreachable_code)]
            let _test = $to_struct {
                $($field: panic!()),*
            };
            #[allow(unreachable_code)]
            let _test = c::$from_struct {
                $($field: panic!()),*
            };
        }

        // Check that the fields have the right sizes and offsets.
        $(check_renamed_struct_field!($to_struct, $from_struct, $field));*
    };
}
