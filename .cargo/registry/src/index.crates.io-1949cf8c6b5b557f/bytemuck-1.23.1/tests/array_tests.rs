#[test]
pub fn test_cast_array() {
  let x = [0u32, 1u32, 2u32];
  let _: [u16; 6] = bytemuck::cast(x);
}

#[cfg(feature = "min_const_generics")]
#[test]
pub fn test_cast_long_array() {
  let x = [0u32; 65];
  let _: [u16; 130] = bytemuck::cast(x);
}
