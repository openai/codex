/// A trait for types that are an array.
///
/// An "array", for our purposes, has the following properties:
/// * Owns some number of elements.
/// * The element type can be generic, but must implement [`Default`].
/// * The capacity is fixed at compile time, based on the implementing type.
/// * You can get a shared or mutable slice to the elements.
///
/// You are generally **not** expected to need to implement this yourself. It is
/// already implemented for all array lengths.
///
/// **Additional lengths can easily be added upon request.**
///
/// ## Safety Reminder
///
/// Just a reminder: this trait is 100% safe, which means that `unsafe` code
/// **must not** rely on an instance of this trait being correct.
pub trait Array {
  /// The type of the items in the thing.
  type Item: Default;

  /// The number of slots in the thing.
  const CAPACITY: usize;

  /// Gives a shared slice over the whole thing.
  ///
  /// A correct implementation will return a slice with a length equal to the
  /// `CAPACITY` value.
  fn as_slice(&self) -> &[Self::Item];

  /// Gives a unique slice over the whole thing.
  ///
  /// A correct implementation will return a slice with a length equal to the
  /// `CAPACITY` value.
  fn as_slice_mut(&mut self) -> &mut [Self::Item];

  /// Create a default-initialized instance of ourself, similar to the
  /// [`Default`] trait, but implemented for the same range of sizes as
  /// [`Array`].
  fn default() -> Self;
}

mod const_generic_impl;

#[cfg(feature = "generic-array")]
mod generic_array_impl;
