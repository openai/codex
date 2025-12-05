package anstyle

// The escape! macro from Rust is converted to a function in Color.kt:
// internal fun escape(vararg parts: String): String = "\u001B[${parts.joinToString("")}m"
//
// In Rust, the macro was:
// macro_rules! escape {
//     ($($inner:expr),*) => {
//         concat!("\x1B[", $($inner),*, "m")
//     };
// }
//
// This file is kept for reference only.
// The escape function is defined in Color.kt since Kotlin doesn't have file-scoped macros.
