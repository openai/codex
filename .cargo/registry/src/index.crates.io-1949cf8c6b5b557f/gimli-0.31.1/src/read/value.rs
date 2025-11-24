//! Definitions for values used in DWARF expressions.

use crate::constants;
#[cfg(feature = "read")]
use crate::read::{AttributeValue, DebuggingInformationEntry};
use crate::read::{Error, Reader, Result};

/// Convert a u64 to an i64, with sign extension if required.
///
/// This is primarily used when needing to treat `Value::Generic`
/// as a signed value.
#[inline]
fn sign_extend(value: u64, mask: u64) -> i64 {
    let value = (value & mask) as i64;
    let sign = ((mask >> 1) + 1) as i64;
    (value ^ sign).wrapping_sub(sign)
}

#[inline]
fn mask_bit_size(addr_mask: u64) -> u32 {
    64 - addr_mask.leading_zeros()
}

/// The type of an entry on the DWARF stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    /// The generic type, which is address-sized and of unspecified sign,
    /// as specified in the DWARF 5 standard, section 2.5.1.
    /// This type is also used to represent address base types.
    Generic,
    /// Signed 8-bit integer type.
    I8,
    /// Unsigned 8-bit integer type.
    U8,
    /// Signed 16-bit integer type.
    I16,
    /// Unsigned 16-bit integer type.
    U16,
    /// Signed 32-bit integer type.
    I32,
    /// Unsigned 32-bit integer type.
    U32,
    /// Signed 64-bit integer type.
    I64,
    /// Unsigned 64-bit integer type.
    U64,
    /// 32-bit floating point type.
    F32,
    /// 64-bit floating point type.
    F64,
}

/// The value of an entry on the DWARF stack.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    /// A generic value, which is address-sized and of unspecified sign.
    Generic(u64),
    /// A signed 8-bit integer value.
    I8(i8),
    /// An unsigned 8-bit integer value.
    U8(u8),
    /// A signed 16-bit integer value.
    I16(i16),
    /// An unsigned 16-bit integer value.
    U16(u16),
    /// A signed 32-bit integer value.
    I32(i32),
    /// An unsigned 32-bit integer value.
    U32(u32),
    /// A signed 64-bit integer value.
    I64(i64),
    /// An unsigned 64-bit integer value.
    U64(u64),
    /// A 32-bit floating point value.
    F32(f32),
    /// A 64-bit floating point value.
    F64(f64),
}

impl ValueType {
    /// The size in bits of a value for this type.
    pub fn bit_size(self, addr_mask: u64) -> u32 {
        match self {
            ValueType::Generic => mask_bit_size(addr_mask),
            ValueType::I8 | ValueType::U8 => 8,
            ValueType::I16 | ValueType::U16 => 16,
            ValueType::I32 | ValueType::U32 | ValueType::F32 => 32,
            ValueType::I64 | ValueType::U64 | ValueType::F64 => 64,
        }
    }

    /// Construct a `ValueType` from the attributes of a base type DIE.
    pub fn from_encoding(encoding: constants::DwAte, byte_size: u64) -> Option<ValueType> {
        Some(match (encoding, byte_size) {
            (constants::DW_ATE_signed, 1) => ValueType::I8,
            (constants::DW_ATE_signed, 2) => ValueType::I16,
            (constants::DW_ATE_signed, 4) => ValueType::I32,
            (constants::DW_ATE_signed, 8) => ValueType::I64,
            (constants::DW_ATE_unsigned, 1) => ValueType::U8,
            (constants::DW_ATE_unsigned, 2) => ValueType::U16,
            (constants::DW_ATE_unsigned, 4) => ValueType::U32,
            (constants::DW_ATE_unsigned, 8) => ValueType::U64,
            (constants::DW_ATE_float, 4) => ValueType::F32,
            (constants::DW_ATE_float, 8) => ValueType::F64,
            _ => return None,
        })
    }

    /// Construct a `ValueType` from a base type DIE.
    #[cfg(feature = "read")]
    pub fn from_entry<R: Reader>(
        entry: &DebuggingInformationEntry<'_, '_, R>,
    ) -> Result<Option<ValueType>> {
        if entry.tag() != constants::DW_TAG_base_type {
            return Ok(None);
        }
        let mut encoding = None;
        let mut byte_size = None;
        let mut endianity = constants::DW_END_default;
        let mut attrs = entry.attrs();
        while let Some(attr) = attrs.next()? {
            match attr.name() {
                constants::DW_AT_byte_size => byte_size = attr.udata_value(),
                constants::DW_AT_encoding => {
                    if let AttributeValue::Encoding(x) = attr.value() {
                        encoding = Some(x);
                    }
                }
                constants::DW_AT_endianity => {
                    if let AttributeValue::Endianity(x) = attr.value() {
                        endianity = x;
                    }
                }
                _ => {}
            }
        }

        if endianity != constants::DW_END_default {
            // TODO: we could check if it matches the reader endianity,
            // but normally it would use DW_END_default in that case.
            return Ok(None);
        }

        if let (Some(encoding), Some(byte_size)) = (encoding, byte_size) {
            Ok(ValueType::from_encoding(encoding, byte_size))
        } else {
            Ok(None)
        }
    }
}

impl Value {
    /// Return the `ValueType` corresponding to this `Value`.
    pub fn value_type(&self) -> ValueType {
        match *self {
            Value::Generic(_) => ValueType::Generic,
            Value::I8(_) => ValueType::I8,
            Value::U8(_) => ValueType::U8,
            Value::I16(_) => ValueType::I16,
            Value::U16(_) => ValueType::U16,
            Value::I32(_) => ValueType::I32,
            Value::U32(_) => ValueType::U32,
            Value::I64(_) => ValueType::I64,
            Value::U64(_) => ValueType::U64,
            Value::F32(_) => ValueType::F32,
            Value::F64(_) => ValueType::F64,
        }
    }

    /// Read a `Value` with the given `value_type` from a `Reader`.
    pub fn parse<R: Reader>(value_type: ValueType, mut bytes: R) -> Result<Value> {
        let value = match value_type {
            ValueType::I8 => Value::I8(bytes.read_i8()?),
            ValueType::U8 => Value::U8(bytes.read_u8()?),
            ValueType::I16 => Value::I16(bytes.read_i16()?),
            ValueType::U16 => Value::U16(bytes.read_u16()?),
            ValueType::I32 => Value::I32(bytes.read_i32()?),
            ValueType::U32 => Value::U32(bytes.read_u32()?),
            ValueType::I64 => Value::I64(bytes.read_i64()?),
            ValueType::U64 => Value::U64(bytes.read_u64()?),
            ValueType::F32 => Value::F32(bytes.read_f32()?),
            ValueType::F64 => Value::F64(bytes.read_f64()?),
            _ => return Err(Error::UnsupportedTypeOperation),
        };
        Ok(value)
    }

    /// Convert a `Value` to a `u64`.
    ///
    /// The `ValueType` of `self` must be integral.
    /// Values are sign extended if the source value is signed.
    pub fn to_u64(self, addr_mask: u64) -> Result<u64> {
        let value = match self {
            Value::Generic(value) => value & addr_mask,
            Value::I8(value) => value as u64,
            Value::U8(value) => u64::from(value),
            Value::I16(value) => value as u64,
            Value::U16(value) => u64::from(value),
            Value::I32(value) => value as u64,
            Value::U32(value) => u64::from(value),
            Value::I64(value) => value as u64,
            Value::U64(value) => value,
            _ => return Err(Error::IntegralTypeRequired),
        };
        Ok(value)
    }

    /// Create a `Value` with the given `value_type` from a `u64` value.
    ///
    /// The `value_type` may be integral or floating point.
    /// The result is truncated if the `u64` value does
    /// not fit the bounds of the `value_type`.
    pub fn from_u64(value_type: ValueType, value: u64) -> Result<Value> {
        let value = match value_type {
            ValueType::Generic => Value::Generic(value),
            ValueType::I8 => Value::I8(value as i8),
            ValueType::U8 => Value::U8(value as u8),
            ValueType::I16 => Value::I16(value as i16),
            ValueType::U16 => Value::U16(value as u16),
            ValueType::I32 => Value::I32(value as i32),
            ValueType::U32 => Value::U32(value as u32),
            ValueType::I64 => Value::I64(value as i64),
            ValueType::U64 => Value::U64(value),
            ValueType::F32 => Value::F32(value as f32),
            ValueType::F64 => Value::F64(value as f64),
        };
        Ok(value)
    }

    /// Create a `Value` with the given `value_type` from a `f32` value.
    ///
    /// The `value_type` may be integral or floating point.
    /// The result is not defined if the `f32` value does
    /// not fit the bounds of the `value_type`.
    fn from_f32(value_type: ValueType, value: f32) -> Result<Value> {
        let value = match value_type {
            ValueType::Generic => Value::Generic(value as u64),
            ValueType::I8 => Value::I8(value as i8),
            ValueType::U8 => Value::U8(value as u8),
            ValueType::I16 => Value::I16(value as i16),
            ValueType::U16 => Value::U16(value as u16),
            ValueType::I32 => Value::I32(value as i32),
            ValueType::U32 => Value::U32(value as u32),
            ValueType::I64 => Value::I64(value as i64),
            ValueType::U64 => Value::U64(value as u64),
            ValueType::F32 => Value::F32(value),
            ValueType::F64 => Value::F64(f64::from(value)),
        };
        Ok(value)
    }

    /// Create a `Value` with the given `value_type` from a `f64` value.
    ///
    /// The `value_type` may be integral or floating point.
    /// The result is not defined if the `f64` value does
    /// not fit the bounds of the `value_type`.
    fn from_f64(value_type: ValueType, value: f64) -> Result<Value> {
        let value = match value_type {
            ValueType::Generic => Value::Generic(value as u64),
            ValueType::I8 => Value::I8(value as i8),
            ValueType::U8 => Value::U8(value as u8),
            ValueType::I16 => Value::I16(value as i16),
            ValueType::U16 => Value::U16(value as u16),
            ValueType::I32 => Value::I32(value as i32),
            ValueType::U32 => Value::U32(value as u32),
            ValueType::I64 => Value::I64(value as i64),
            ValueType::U64 => Value::U64(value as u64),
            ValueType::F32 => Value::F32(value as f32),
            ValueType::F64 => Value::F64(value),
        };
        Ok(value)
    }

    /// Convert a `Value` to the given `value_type`.
    ///
    /// When converting between integral types, the result is truncated
    /// if the source value does not fit the bounds of the `value_type`.
    /// When converting from floating point types, the result is not defined
    /// if the source value does not fit the bounds of the `value_type`.
    ///
    /// This corresponds to the DWARF `DW_OP_convert` operation.
    pub fn convert(self, value_type: ValueType, addr_mask: u64) -> Result<Value> {
        match self {
            Value::F32(value) => Value::from_f32(value_type, value),
            Value::F64(value) => Value::from_f64(value_type, value),
            _ => Value::from_u64(value_type, self.to_u64(addr_mask)?),
        }
    }

    /// Reinterpret the bits in a `Value` as the given `value_type`.
    ///
    /// The source and result value types must have equal sizes.
    ///
    /// This corresponds to the DWARF `DW_OP_reinterpret` operation.
    pub fn reinterpret(self, value_type: ValueType, addr_mask: u64) -> Result<Value> {
        if self.value_type().bit_size(addr_mask) != value_type.bit_size(addr_mask) {
            return Err(Error::TypeMismatch);
        }
        let bits = match self {
            Value::Generic(value) => value,
            Value::I8(value) => value as u64,
            Value::U8(value) => u64::from(value),
            Value::I16(value) => value as u64,
            Value::U16(value) => u64::from(value),
            Value::I32(value) => value as u64,
            Value::U32(value) => u64::from(value),
            Value::I64(value) => value as u64,
            Value::U64(value) => value,
            Value::F32(value) => u64::from(f32::to_bits(value)),
            Value::F64(value) => f64::to_bits(value),
        };
        let value = match value_type {
            ValueType::Generic => Value::Generic(bits),
            ValueType::I8 => Value::I8(bits as i8),
            ValueType::U8 => Value::U8(bits as u8),
            ValueType::I16 => Value::I16(bits as i16),
            ValueType::U16 => Value::U16(bits as u16),
            ValueType::I32 => Value::I32(bits as i32),
            ValueType::U32 => Value::U32(bits as u32),
            ValueType::I64 => Value::I64(bits as i64),
            ValueType::U64 => Value::U64(bits),
            ValueType::F32 => Value::F32(f32::from_bits(bits as u32)),
            ValueType::F64 => Value::F64(f64::from_bits(bits)),
        };
        Ok(value)
    }

    /// Perform an absolute value operation.
    ///
    /// If the value type is `Generic`, then it is interpreted as a signed value.
    ///
    /// This corresponds to the DWARF `DW_OP_abs` operation.
    pub fn abs(self, addr_mask: u64) -> Result<Value> {
        // wrapping_abs() can be used because DWARF specifies that the result is undefined
        // for negative minimal values.
        let value = match self {
            Value::Generic(value) => {
                Value::Generic(sign_extend(value, addr_mask).wrapping_abs() as u64)
            }
            Value::I8(value) => Value::I8(value.wrapping_abs()),
            Value::I16(value) => Value::I16(value.wrapping_abs()),
            Value::I32(value) => Value::I32(value.wrapping_abs()),
            Value::I64(value) => Value::I64(value.wrapping_abs()),
            // f32/f64::abs() is not available in libcore
            Value::F32(value) => Value::F32(if value < 0. { -value } else { value }),
            Value::F64(value) => Value::F64(if value < 0. { -value } else { value }),
            Value::U8(_) | Value::U16(_) | Value::U32(_) | Value::U64(_) => self,
        };
        Ok(value)
    }

    /// Perform a negation operation.
    ///
    /// If the value type is `Generic`, then it is interpreted as a signed value.
    ///
    /// This corresponds to the DWARF `DW_OP_neg` operation.
    pub fn neg(self, addr_mask: u64) -> Result<Value> {
        // wrapping_neg() can be used because DWARF specifies that the result is undefined
        // for negative minimal values.
        let value = match self {
            Value::Generic(value) => {
                Value::Generic(sign_extend(value, addr_mask).wrapping_neg() as u64)
            }
            Value::I8(value) => Value::I8(value.wrapping_neg()),
            Value::I16(value) => Value::I16(value.wrapping_neg()),
            Value::I32(value) => Value::I32(value.wrapping_neg()),
            Value::I64(value) => Value::I64(value.wrapping_neg()),
            Value::F32(value) => Value::F32(-value),
            Value::F64(value) => Value::F64(-value),
            // It's unclear if these should implicitly convert to a signed value.
            // For now, we don't support them.
            Value::U8(_) | Value::U16(_) | Value::U32(_) | Value::U64(_) => {
                return Err(Error::UnsupportedTypeOperation);
            }
        };
        Ok(value)
    }

    /// Perform an addition operation.
    ///
    /// This operation requires matching types.
    ///
    /// This corresponds to the DWARF `DW_OP_plus` operation.
    pub fn add(self, rhs: Value, addr_mask: u64) -> Result<Value> {
        let value = match (self, rhs) {
            (Value::Generic(v1), Value::Generic(v2)) => {
                Value::Generic(v1.wrapping_add(v2) & addr_mask)
            }
            (Value::I8(v1), Value::I8(v2)) => Value::I8(v1.wrapping_add(v2)),
            (Value::U8(v1), Value::U8(v2)) => Value::U8(v1.wrapping_add(v2)),
            (Value::I16(v1), Value::I16(v2)) => Value::I16(v1.wrapping_add(v2)),
            (Value::U16(v1), Value::U16(v2)) => Value::U16(v1.wrapping_add(v2)),
            (Value::I32(v1), Value::I32(v2)) => Value::I32(v1.wrapping_add(v2)),
            (Value::U32(v1), Value::U32(v2)) => Value::U32(v1.wrapping_add(v2)),
            (Value::I64(v1), Value::I64(v2)) => Value::I64(v1.wrapping_add(v2)),
            (Value::U64(v1), Value::U64(v2)) => Value::U64(v1.wrapping_add(v2)),
            (Value::F32(v1), Value::F32(v2)) => Value::F32(v1 + v2),
            (Value::F64(v1), Value::F64(v2)) => Value::F64(v1 + v2),
            _ => return Err(Error::TypeMismatch),
        };
        Ok(value)
    }

    /// Perform a subtraction operation.
    ///
    /// This operation requires matching types.
    ///
    /// This corresponds to the DWARF `DW_OP_minus` operation.
    pub fn sub(self, rhs: Value, addr_mask: u64) -> Result<Value> {
        let value = match (self, rhs) {
            (Value::Generic(v1), Value::Generic(v2)) => {
                Value::Generic(v1.wrapping_sub(v2) & addr_mask)
            }
            (Value::I8(v1), Value::I8(v2)) => Value::I8(v1.wrapping_sub(v2)),
            (Value::U8(v1), Value::U8(v2)) => Value::U8(v1.wrapping_sub(v2)),
            (Value::I16(v1), Value::I16(v2)) => Value::I16(v1.wrapping_sub(v2)),
            (Value::U16(v1), Value::U16(v2)) => Value::U16(v1.wrapping_sub(v2)),
            (Value::I32(v1), Value::I32(v2)) => Value::I32(v1.wrapping_sub(v2)),
            (Value::U32(v1), Value::U32(v2)) => Value::U32(v1.wrapping_sub(v2)),
            (Value::I64(v1), Value::I64(v2)) => Value::I64(v1.wrapping_sub(v2)),
            (Value::U64(v1), Value::U64(v2)) => Value::U64(v1.wrapping_sub(v2)),
            (Value::F32(v1), Value::F32(v2)) => Value::F32(v1 - v2),
            (Value::F64(v1), Value::F64(v2)) => Value::F64(v1 - v2),
            _ => return Err(Error::TypeMismatch),
        };
        Ok(value)
    }

    /// Perform a multiplication operation.
    ///
    /// This operation requires matching types.
    ///
    /// This corresponds to the DWARF `DW_OP_mul` operation.
    pub fn mul(self, rhs: Value, addr_mask: u64) -> Result<Value> {
        let value = match (self, rhs) {
            (Value::Generic(v1), Value::Generic(v2)) => {
                Value::Generic(v1.wrapping_mul(v2) & addr_mask)
            }
            (Value::I8(v1), Value::I8(v2)) => Value::I8(v1.wrapping_mul(v2)),
            (Value::U8(v1), Value::U8(v2)) => Value::U8(v1.wrapping_mul(v2)),
            (Value::I16(v1), Value::I16(v2)) => Value::I16(v1.wrapping_mul(v2)),
            (Value::U16(v1), Value::U16(v2)) => Value::U16(v1.wrapping_mul(v2)),
            (Value::I32(v1), Value::I32(v2)) => Value::I32(v1.wrapping_mul(v2)),
            (Value::U32(v1), Value::U32(v2)) => Value::U32(v1.wrapping_mul(v2)),
            (Value::I64(v1), Value::I64(v2)) => Value::I64(v1.wrapping_mul(v2)),
            (Value::U64(v1), Value::U64(v2)) => Value::U64(v1.wrapping_mul(v2)),
            (Value::F32(v1), Value::F32(v2)) => Value::F32(v1 * v2),
            (Value::F64(v1), Value::F64(v2)) => Value::F64(v1 * v2),
            _ => return Err(Error::TypeMismatch),
        };
        Ok(value)
    }

    /// Perform a division operation.
    ///
    /// This operation requires matching types.
    /// If the value type is `Generic`, then it is interpreted as a signed value.
    ///
    /// This corresponds to the DWARF `DW_OP_div` operation.
    pub fn div(self, rhs: Value, addr_mask: u64) -> Result<Value> {
        match rhs {
            Value::Generic(v2) if sign_extend(v2, addr_mask) == 0 => {
                return Err(Error::DivisionByZero);
            }
            Value::I8(0)
            | Value::U8(0)
            | Value::I16(0)
            | Value::U16(0)
            | Value::I32(0)
            | Value::U32(0)
            | Value::I64(0)
            | Value::U64(0) => {
                return Err(Error::DivisionByZero);
            }
            _ => {}
        }
        let value = match (self, rhs) {
            (Value::Generic(v1), Value::Generic(v2)) => {
                // Signed division
                Value::Generic(
                    sign_extend(v1, addr_mask).wrapping_div(sign_extend(v2, addr_mask)) as u64,
                )
            }
            (Value::I8(v1), Value::I8(v2)) => Value::I8(v1.wrapping_div(v2)),
            (Value::U8(v1), Value::U8(v2)) => Value::U8(v1.wrapping_div(v2)),
            (Value::I16(v1), Value::I16(v2)) => Value::I16(v1.wrapping_div(v2)),
            (Value::U16(v1), Value::U16(v2)) => Value::U16(v1.wrapping_div(v2)),
            (Value::I32(v1), Value::I32(v2)) => Value::I32(v1.wrapping_div(v2)),
            (Value::U32(v1), Value::U32(v2)) => Value::U32(v1.wrapping_div(v2)),
            (Value::I64(v1), Value::I64(v2)) => Value::I64(v1.wrapping_div(v2)),
            (Value::U64(v1), Value::U64(v2)) => Value::U64(v1.wrapping_div(v2)),
            (Value::F32(v1), Value::F32(v2)) => Value::F32(v1 / v2),
            (Value::F64(v1), Value::F64(v2)) => Value::F64(v1 / v2),
            _ => return Err(Error::TypeMismatch),
        };
        Ok(value)
    }

    /// Perform a remainder operation.
    ///
    /// This operation requires matching integral types.
    /// If the value type is `Generic`, then it is interpreted as an unsigned value.
    ///
    /// This corresponds to the DWARF `DW_OP_mod` operation.
    pub fn rem(self, rhs: Value, addr_mask: u64) -> Result<Value> {
        match rhs {
            Value::Generic(rhs) if (rhs & addr_mask) == 0 => {
                return Err(Error::DivisionByZero);
            }
            Value::I8(0)
            | Value::U8(0)
            | Value::I16(0)
            | Value::U16(0)
            | Value::I32(0)
            | Value::U32(0)
            | Value::I64(0)
            | Value::U64(0) => {
                return Err(Error::DivisionByZero);
            }
            _ => {}
        }
        let value = match (self, rhs) {
            (Value::Generic(v1), Value::Generic(v2)) => {
                // Unsigned modulus
                Value::Generic((v1 & addr_mask).wrapping_rem(v2 & addr_mask))
            }
            (Value::I8(v1), Value::I8(v2)) => Value::I8(v1.wrapping_rem(v2)),
            (Value::U8(v1), Value::U8(v2)) => Value::U8(v1.wrapping_rem(v2)),
            (Value::I16(v1), Value::I16(v2)) => Value::I16(v1.wrapping_rem(v2)),
            (Value::U16(v1), Value::U16(v2)) => Value::U16(v1.wrapping_rem(v2)),
            (Value::I32(v1), Value::I32(v2)) => Value::I32(v1.wrapping_rem(v2)),
            (Value::U32(v1), Value::U32(v2)) => Value::U32(v1.wrapping_rem(v2)),
            (Value::I64(v1), Value::I64(v2)) => Value::I64(v1.wrapping_rem(v2)),
            (Value::U64(v1), Value::U64(v2)) => Value::U64(v1.wrapping_rem(v2)),
            (Value::F32(_), Value::F32(_)) => return Err(Error::IntegralTypeRequired),
            (Value::F64(_), Value::F64(_)) => return Err(Error::IntegralTypeRequired),
            _ => return Err(Error::TypeMismatch),
        };
        Ok(value)
    }

    /// Perform a bitwise not operation.
    ///
    /// This operation requires matching integral types.
    ///
    /// This corresponds to the DWARF `DW_OP_not` operation.
    pub fn not(self, addr_mask: u64) -> Result<Value> {
        let value_type = self.value_type();
        let v = self.to_u64(addr_mask)?;
        Value::from_u64(value_type, !v)
    }

    /// Perform a bitwise and operation.
    ///
    /// This operation requires matching integral types.
    ///
    /// This corresponds to the DWARF `DW_OP_and` operation.
    pub fn and(self, rhs: Value, addr_mask: u64) -> Result<Value> {
        let value_type = self.value_type();
        if value_type != rhs.value_type() {
            return Err(Error::TypeMismatch);
        }
        let v1 = self.to_u64(addr_mask)?;
        let v2 = rhs.to_u64(addr_mask)?;
        Value::from_u64(value_type, v1 & v2)
    }

    /// Perform a bitwise or operation.
    ///
    /// This operation requires matching integral types.
    ///
    /// This corresponds to the DWARF `DW_OP_or` operation.
    pub fn or(self, rhs: Value, addr_mask: u64) -> Result<Value> {
        let value_type = self.value_type();
        if value_type != rhs.value_type() {
            return Err(Error::TypeMismatch);
        }
        let v1 = self.to_u64(addr_mask)?;
        let v2 = rhs.to_u64(addr_mask)?;
        Value::from_u64(value_type, v1 | v2)
    }

    /// Perform a bitwise exclusive-or operation.
    ///
    /// This operation requires matching integral types.
    ///
    /// This corresponds to the DWARF `DW_OP_xor` operation.
    pub fn xor(self, rhs: Value, addr_mask: u64) -> Result<Value> {
        let value_type = self.value_type();
        if value_type != rhs.value_type() {
            return Err(Error::TypeMismatch);
        }
        let v1 = self.to_u64(addr_mask)?;
        let v2 = rhs.to_u64(addr_mask)?;
        Value::from_u64(value_type, v1 ^ v2)
    }

    /// Convert value to bit length suitable for a shift operation.
    ///
    /// If the value is negative then an error is returned.
    fn shift_length(self) -> Result<u64> {
        let value = match self {
            Value::Generic(value) => value,
            Value::I8(value) if value >= 0 => value as u64,
            Value::U8(value) => u64::from(value),
            Value::I16(value) if value >= 0 => value as u64,
            Value::U16(value) => u64::from(value),
            Value::I32(value) if value >= 0 => value as u64,
            Value::U32(value) => u64::from(value),
            Value::I64(value) if value >= 0 => value as u64,
            Value::U64(value) => value,
            _ => return Err(Error::InvalidShiftExpression),
        };
        Ok(value)
    }

    /// Perform a shift left operation.
    ///
    /// This operation requires integral types.
    /// If the shift length exceeds the type size, then 0 is returned.
    /// If the shift length is negative then an error is returned.
    ///
    /// This corresponds to the DWARF `DW_OP_shl` operation.
    pub fn shl(self, rhs: Value, addr_mask: u64) -> Result<Value> {
        let v2 = rhs.shift_length()?;
        let value = match self {
            Value::Generic(v1) => Value::Generic(if v2 >= u64::from(mask_bit_size(addr_mask)) {
                0
            } else {
                (v1 & addr_mask) << v2
            }),
            Value::I8(v1) => Value::I8(if v2 >= 8 { 0 } else { v1 << v2 }),
            Value::U8(v1) => Value::U8(if v2 >= 8 { 0 } else { v1 << v2 }),
            Value::I16(v1) => Value::I16(if v2 >= 16 { 0 } else { v1 << v2 }),
            Value::U16(v1) => Value::U16(if v2 >= 16 { 0 } else { v1 << v2 }),
            Value::I32(v1) => Value::I32(if v2 >= 32 { 0 } else { v1 << v2 }),
            Value::U32(v1) => Value::U32(if v2 >= 32 { 0 } else { v1 << v2 }),
            Value::I64(v1) => Value::I64(if v2 >= 64 { 0 } else { v1 << v2 }),
            Value::U64(v1) => Value::U64(if v2 >= 64 { 0 } else { v1 << v2 }),
            _ => return Err(Error::IntegralTypeRequired),
        };
        Ok(value)
    }

    /// Perform a logical shift right operation.
    ///
    /// This operation requires an unsigned integral type for the value.
    /// If the value type is `Generic`, then it is interpreted as an unsigned value.
    ///
    /// This operation requires an integral type for the shift length.
    /// If the shift length exceeds the type size, then 0 is returned.
    /// If the shift length is negative then an error is returned.
    ///
    /// This corresponds to the DWARF `DW_OP_shr` operation.
    pub fn shr(self, rhs: Value, addr_mask: u64) -> Result<Value> {
        let v2 = rhs.shift_length()?;
        let value = match self {
            Value::Generic(v1) => Value::Generic(if v2 >= u64::from(mask_bit_size(addr_mask)) {
                0
            } else {
                (v1 & addr_mask) >> v2
            }),
            Value::U8(v1) => Value::U8(if v2 >= 8 { 0 } else { v1 >> v2 }),
            Value::U16(v1) => Value::U16(if v2 >= 16 { 0 } else { v1 >> v2 }),
            Value::U32(v1) => Value::U32(if v2 >= 32 { 0 } else { v1 >> v2 }),
            Value::U64(v1) => Value::U64(if v2 >= 64 { 0 } else { v1 >> v2 }),
            // It's unclear if signed values should implicitly convert to an unsigned value.
            // For now, we don't support them.
            Value::I8(_) | Value::I16(_) | Value::I32(_) | Value::I64(_) => {
                return Err(Error::UnsupportedTypeOperation);
            }
            _ => return Err(Error::IntegralTypeRequired),
        };
        Ok(value)
    }

    /// Perform an arithmetic shift right operation.
    ///
    /// This operation requires a signed integral type for the value.
    /// If the value type is `Generic`, then it is interpreted as a signed value.
    ///
    /// This operation requires an integral type for the shift length.
    /// If the shift length exceeds the type size, then 0 is returned for positive values,
    /// and -1 is returned for negative values.
    /// If the shift length is negative then an error is returned.
    ///
    /// This corresponds to the DWARF `DW_OP_shra` operation.
    pub fn shra(self, rhs: Value, addr_mask: u64) -> Result<Value> {
        let v2 = rhs.shift_length()?;
        let value = match self {
            Value::Generic(v1) => {
                let v1 = sign_extend(v1, addr_mask);
                let value = if v2 >= u64::from(mask_bit_size(addr_mask)) {
                    if v1 < 0 {
                        !0
                    } else {
                        0
                    }
                } else {
                    (v1 >> v2) as u64
                };
                Value::Generic(value)
            }
            Value::I8(v1) => Value::I8(if v2 >= 8 {
                if v1 < 0 {
                    !0
                } else {
                    0
                }
            } else {
                v1 >> v2
            }),
            Value::I16(v1) => Value::I16(if v2 >= 16 {
                if v1 < 0 {
                    !0
                } else {
                    0
                }
            } else {
                v1 >> v2
            }),
            Value::I32(v1) => Value::I32(if v2 >= 32 {
                if v1 < 0 {
                    !0
                } else {
                    0
                }
            } else {
                v1 >> v2
            }),
            Value::I64(v1) => Value::I64(if v2 >= 64 {
                if v1 < 0 {
                    !0
                } else {
                    0
                }
            } else {
                v1 >> v2
            }),
            // It's unclear if unsigned values should implicitly convert to a signed value.
            // For now, we don't support them.
            Value::U8(_) | Value::U16(_) | Value::U32(_) | Value::U64(_) => {
                return Err(Error::UnsupportedTypeOperation);
            }
            _ => return Err(Error::IntegralTypeRequired),
        };
        Ok(value)
    }

    /// Perform the `==` relational operation.
    ///
    /// This operation requires matching integral types.
    /// If the value type is `Generic`, then it is interpreted as a signed value.
    ///
    /// This corresponds to the DWARF `DW_OP_eq` operation.
    pub fn eq(self, rhs: Value, addr_mask: u64) -> Result<Value> {
        let value = match (self, rhs) {
            (Value::Generic(v1), Value::Generic(v2)) => {
                sign_extend(v1, addr_mask) == sign_extend(v2, addr_mask)
            }
            (Value::I8(v1), Value::I8(v2)) => v1 == v2,
            (Value::U8(v1), Value::U8(v2)) => v1 == v2,
            (Value::I16(v1), Value::I16(v2)) => v1 == v2,
            (Value::U16(v1), Value::U16(v2)) => v1 == v2,
            (Value::I32(v1), Value::I32(v2)) => v1 == v2,
            (Value::U32(v1), Value::U32(v2)) => v1 == v2,
            (Value::I64(v1), Value::I64(v2)) => v1 == v2,
            (Value::U64(v1), Value::U64(v2)) => v1 == v2,
            (Value::F32(v1), Value::F32(v2)) => v1 == v2,
            (Value::F64(v1), Value::F64(v2)) => v1 == v2,
            _ => return Err(Error::TypeMismatch),
        };
        Ok(Value::Generic(value as u64))
    }

    /// Perform the `>=` relational operation.
    ///
    /// This operation requires matching integral types.
    /// If the value type is `Generic`, then it is interpreted as a signed value.
    ///
    /// This corresponds to the DWARF `DW_OP_ge` operation.
    pub fn ge(self, rhs: Value, addr_mask: u64) -> Result<Value> {
        let value = match (self, rhs) {
            (Value::Generic(v1), Value::Generic(v2)) => {
                sign_extend(v1, addr_mask) >= sign_extend(v2, addr_mask)
            }
            (Value::I8(v1), Value::I8(v2)) => v1 >= v2,
            (Value::U8(v1), Value::U8(v2)) => v1 >= v2,
            (Value::I16(v1), Value::I16(v2)) => v1 >= v2,
            (Value::U16(v1), Value::U16(v2)) => v1 >= v2,
            (Value::I32(v1), Value::I32(v2)) => v1 >= v2,
            (Value::U32(v1), Value::U32(v2)) => v1 >= v2,
            (Value::I64(v1), Value::I64(v2)) => v1 >= v2,
            (Value::U64(v1), Value::U64(v2)) => v1 >= v2,
            (Value::F32(v1), Value::F32(v2)) => v1 >= v2,
            (Value::F64(v1), Value::F64(v2)) => v1 >= v2,
            _ => return Err(Error::TypeMismatch),
        };
        Ok(Value::Generic(value as u64))
    }

    /// Perform the `>` relational operation.
    ///
    /// This operation requires matching integral types.
    /// If the value type is `Generic`, then it is interpreted as a signed value.
    ///
    /// This corresponds to the DWARF `DW_OP_gt` operation.
    pub fn gt(self, rhs: Value, addr_mask: u64) -> Result<Value> {
        let value = match (self, rhs) {
            (Value::Generic(v1), Value::Generic(v2)) => {
                sign_extend(v1, addr_mask) > sign_extend(v2, addr_mask)
            }
            (Value::I8(v1), Value::I8(v2)) => v1 > v2,
            (Value::U8(v1), Value::U8(v2)) => v1 > v2,
            (Value::I16(v1), Value::I16(v2)) => v1 > v2,
            (Value::U16(v1), Value::U16(v2)) => v1 > v2,
            (Value::I32(v1), Value::I32(v2)) => v1 > v2,
            (Value::U32(v1), Value::U32(v2)) => v1 > v2,
            (Value::I64(v1), Value::I64(v2)) => v1 > v2,
            (Value::U64(v1), Value::U64(v2)) => v1 > v2,
            (Value::F32(v1), Value::F32(v2)) => v1 > v2,
            (Value::F64(v1), Value::F64(v2)) => v1 > v2,
            _ => return Err(Error::TypeMismatch),
        };
        Ok(Value::Generic(value as u64))
    }

    /// Perform the `<= relational operation.
    ///
    /// This operation requires matching integral types.
    /// If the value type is `Generic`, then it is interpreted as a signed value.
    ///
    /// This corresponds to the DWARF `DW_OP_le` operation.
    pub fn le(self, rhs: Value, addr_mask: u64) -> Result<Value> {
        let value = match (self, rhs) {
            (Value::Generic(v1), Value::Generic(v2)) => {
                sign_extend(v1, addr_mask) <= sign_extend(v2, addr_mask)
            }
            (Value::I8(v1), Value::I8(v2)) => v1 <= v2,
            (Value::U8(v1), Value::U8(v2)) => v1 <= v2,
            (Value::I16(v1), Value::I16(v2)) => v1 <= v2,
            (Value::U16(v1), Value::U16(v2)) => v1 <= v2,
            (Value::I32(v1), Value::I32(v2)) => v1 <= v2,
            (Value::U32(v1), Value::U32(v2)) => v1 <= v2,
            (Value::I64(v1), Value::I64(v2)) => v1 <= v2,
            (Value::U64(v1), Value::U64(v2)) => v1 <= v2,
            (Value::F32(v1), Value::F32(v2)) => v1 <= v2,
            (Value::F64(v1), Value::F64(v2)) => v1 <= v2,
            _ => return Err(Error::TypeMismatch),
        };
        Ok(Value::Generic(value as u64))
    }

    /// Perform the `< relational operation.
    ///
    /// This operation requires matching integral types.
    /// If the value type is `Generic`, then it is interpreted as a signed value.
    ///
    /// This corresponds to the DWARF `DW_OP_lt` operation.
    pub fn lt(self, rhs: Value, addr_mask: u64) -> Result<Value> {
        let value = match (self, rhs) {
            (Value::Generic(v1), Value::Generic(v2)) => {
                sign_extend(v1, addr_mask) < sign_extend(v2, addr_mask)
            }
            (Value::I8(v1), Value::I8(v2)) => v1 < v2,
            (Value::U8(v1), Value::U8(v2)) => v1 < v2,
            (Value::I16(v1), Value::I16(v2)) => v1 < v2,
            (Value::U16(v1), Value::U16(v2)) => v1 < v2,
            (Value::I32(v1), Value::I32(v2)) => v1 < v2,
            (Value::U32(v1), Value::U32(v2)) => v1 < v2,
            (Value::I64(v1), Value::I64(v2)) => v1 < v2,
            (Value::U64(v1), Value::U64(v2)) => v1 < v2,
            (Value::F32(v1), Value::F32(v2)) => v1 < v2,
            (Value::F64(v1), Value::F64(v2)) => v1 < v2,
            _ => return Err(Error::TypeMismatch),
        };
        Ok(Value::Generic(value as u64))
    }

    /// Perform the `!= relational operation.
    ///
    /// This operation requires matching integral types.
    /// If the value type is `Generic`, then it is interpreted as a signed value.
    ///
    /// This corresponds to the DWARF `DW_OP_ne` operation.
    pub fn ne(self, rhs: Value, addr_mask: u64) -> Result<Value> {
        let value = match (self, rhs) {
            (Value::Generic(v1), Value::Generic(v2)) => {
                sign_extend(v1, addr_mask) != sign_extend(v2, addr_mask)
            }
            (Value::I8(v1), Value::I8(v2)) => v1 != v2,
            (Value::U8(v1), Value::U8(v2)) => v1 != v2,
            (Value::I16(v1), Value::I16(v2)) => v1 != v2,
            (Value::U16(v1), Value::U16(v2)) => v1 != v2,
            (Value::I32(v1), Value::I32(v2)) => v1 != v2,
            (Value::U32(v1), Value::U32(v2)) => v1 != v2,
            (Value::I64(v1), Value::I64(v2)) => v1 != v2,
            (Value::U64(v1), Value::U64(v2)) => v1 != v2,
            (Value::F32(v1), Value::F32(v2)) => v1 != v2,
            (Value::F64(v1), Value::F64(v2)) => v1 != v2,
            _ => return Err(Error::TypeMismatch),
        };
        Ok(Value::Generic(value as u64))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{DebugAbbrevOffset, DebugInfoOffset, Encoding, Format};
    use crate::endianity::LittleEndian;
    use crate::read::{
        Abbreviation, AttributeSpecification, DebuggingInformationEntry, EndianSlice, UnitHeader,
        UnitOffset, UnitType,
    };

    #[test]
    #[rustfmt::skip]
    fn valuetype_from_encoding() {
        let encoding = Encoding {
            format: Format::Dwarf32,
            version: 4,
            address_size: 4,
        };
        let unit = UnitHeader::new(
            encoding,
            7,
            UnitType::Compilation,
            DebugAbbrevOffset(0),
            DebugInfoOffset(0).into(),
            EndianSlice::new(&[], LittleEndian),
        );

        let abbrev = Abbreviation::new(
            42,
            constants::DW_TAG_base_type,
            constants::DW_CHILDREN_no,
            vec![
                AttributeSpecification::new(
                    constants::DW_AT_byte_size,
                    constants::DW_FORM_udata,
                    None,
                ),
                AttributeSpecification::new(
                    constants::DW_AT_encoding,
                    constants::DW_FORM_udata,
                    None,
                ),
                AttributeSpecification::new(
                    constants::DW_AT_endianity,
                    constants::DW_FORM_udata,
                    None,
                ),
            ].into(),
        );

        for &(attrs, result) in &[
            ([0x01, constants::DW_ATE_signed.0, constants::DW_END_default.0], ValueType::I8),
            ([0x02, constants::DW_ATE_signed.0, constants::DW_END_default.0], ValueType::I16),
            ([0x04, constants::DW_ATE_signed.0, constants::DW_END_default.0], ValueType::I32),
            ([0x08, constants::DW_ATE_signed.0, constants::DW_END_default.0], ValueType::I64),
            ([0x01, constants::DW_ATE_unsigned.0, constants::DW_END_default.0], ValueType::U8),
            ([0x02, constants::DW_ATE_unsigned.0, constants::DW_END_default.0], ValueType::U16),
            ([0x04, constants::DW_ATE_unsigned.0, constants::DW_END_default.0], ValueType::U32),
            ([0x08, constants::DW_ATE_unsigned.0, constants::DW_END_default.0], ValueType::U64),
            ([0x04, constants::DW_ATE_float.0, constants::DW_END_default.0], ValueType::F32),
            ([0x08, constants::DW_ATE_float.0, constants::DW_END_default.0], ValueType::F64),
        ] {
            let entry = DebuggingInformationEntry::new(
                UnitOffset(0),
                EndianSlice::new(&attrs, LittleEndian),
                &abbrev,
                &unit,
            );
            assert_eq!(ValueType::from_entry(&entry), Ok(Some(result)));
        }

        for attrs in &[
            [0x03, constants::DW_ATE_signed.0, constants::DW_END_default.0],
            [0x02, constants::DW_ATE_signed.0, constants::DW_END_big.0],
        ] {
            let entry = DebuggingInformationEntry::new(
                UnitOffset(0),
                EndianSlice::new(attrs, LittleEndian),
                &abbrev,
                &unit,
            );
            assert_eq!(ValueType::from_entry(&entry), Ok(None));
        }
    }

    #[test]
    fn value_convert() {
        let addr_mask = !0 >> 32;
        for &(v, t, result) in &[
            (Value::Generic(1), ValueType::I8, Ok(Value::I8(1))),
            (Value::I8(1), ValueType::U8, Ok(Value::U8(1))),
            (Value::U8(1), ValueType::I16, Ok(Value::I16(1))),
            (Value::I16(1), ValueType::U16, Ok(Value::U16(1))),
            (Value::U16(1), ValueType::I32, Ok(Value::I32(1))),
            (Value::I32(1), ValueType::U32, Ok(Value::U32(1))),
            (Value::U32(1), ValueType::F32, Ok(Value::F32(1.))),
            (Value::F32(1.), ValueType::I64, Ok(Value::I64(1))),
            (Value::I64(1), ValueType::U64, Ok(Value::U64(1))),
            (Value::U64(1), ValueType::F64, Ok(Value::F64(1.))),
            (Value::F64(1.), ValueType::Generic, Ok(Value::Generic(1))),
        ] {
            assert_eq!(v.convert(t, addr_mask), result);
        }
    }

    #[test]
    #[rustfmt::skip]
    fn value_reinterpret() {
        let addr_mask = !0 >> 32;
        for &(v, t, result) in &[
            // 8-bit
            (Value::I8(-1), ValueType::U8, Ok(Value::U8(0xff))),
            (Value::U8(0xff), ValueType::I8, Ok(Value::I8(-1))),
            // 16-bit
            (Value::I16(1), ValueType::U16, Ok(Value::U16(1))),
            (Value::U16(1), ValueType::I16, Ok(Value::I16(1))),
            // 32-bit
            (Value::Generic(1), ValueType::I32, Ok(Value::I32(1))),
            (Value::I32(1), ValueType::U32, Ok(Value::U32(1))),
            (Value::U32(0x3f80_0000), ValueType::F32, Ok(Value::F32(1.0))),
            (Value::F32(1.0), ValueType::Generic, Ok(Value::Generic(0x3f80_0000))),
            // Type mismatches
            (Value::Generic(1), ValueType::U8, Err(Error::TypeMismatch)),
            (Value::U8(1), ValueType::U16, Err(Error::TypeMismatch)),
            (Value::U16(1), ValueType::U32, Err(Error::TypeMismatch)),
            (Value::U32(1), ValueType::U64, Err(Error::TypeMismatch)),
            (Value::U64(1), ValueType::Generic, Err(Error::TypeMismatch)),
        ] {
            assert_eq!(v.reinterpret(t, addr_mask), result);
        }

        let addr_mask = !0;
        for &(v, t, result) in &[
            // 64-bit
            (Value::Generic(1), ValueType::I64, Ok(Value::I64(1))),
            (Value::I64(1), ValueType::U64, Ok(Value::U64(1))),
            (Value::U64(0x3ff0_0000_0000_0000), ValueType::F64, Ok(Value::F64(1.0))),
            (Value::F64(1.0), ValueType::Generic, Ok(Value::Generic(0x3ff0_0000_0000_0000))),
        ] {
            assert_eq!(v.reinterpret(t, addr_mask), result);
        }
    }

    #[test]
    #[rustfmt::skip]
    fn value_abs() {
        let addr_mask = 0xffff_ffff;
        for &(v, result) in &[
            (Value::Generic(0xffff_ffff), Ok(Value::Generic(1))),
            (Value::I8(-1), Ok(Value::I8(1))),
            (Value::U8(1), Ok(Value::U8(1))),
            (Value::I16(-1), Ok(Value::I16(1))),
            (Value::U16(1), Ok(Value::U16(1))),
            (Value::I32(-1), Ok(Value::I32(1))),
            (Value::U32(1), Ok(Value::U32(1))),
            (Value::I64(-1), Ok(Value::I64(1))),
            (Value::U64(1), Ok(Value::U64(1))),
            (Value::F32(-1.), Ok(Value::F32(1.))),
            (Value::F64(-1.), Ok(Value::F64(1.))),
        ] {
            assert_eq!(v.abs(addr_mask), result);
        }
    }

    #[test]
    #[rustfmt::skip]
    fn value_neg() {
        let addr_mask = 0xffff_ffff;
        for &(v, result) in &[
            (Value::Generic(0xffff_ffff), Ok(Value::Generic(1))),
            (Value::I8(1), Ok(Value::I8(-1))),
            (Value::U8(1), Err(Error::UnsupportedTypeOperation)),
            (Value::I16(1), Ok(Value::I16(-1))),
            (Value::U16(1), Err(Error::UnsupportedTypeOperation)),
            (Value::I32(1), Ok(Value::I32(-1))),
            (Value::U32(1), Err(Error::UnsupportedTypeOperation)),
            (Value::I64(1), Ok(Value::I64(-1))),
            (Value::U64(1), Err(Error::UnsupportedTypeOperation)),
            (Value::F32(1.), Ok(Value::F32(-1.))),
            (Value::F64(1.), Ok(Value::F64(-1.))),
        ] {
            assert_eq!(v.neg(addr_mask), result);
        }
    }

    #[test]
    #[rustfmt::skip]
    fn value_add() {
        let addr_mask = 0xffff_ffff;
        for &(v1, v2, result) in &[
            (Value::Generic(1), Value::Generic(2), Ok(Value::Generic(3))),
            (Value::I8(-1), Value::I8(2), Ok(Value::I8(1))),
            (Value::U8(1), Value::U8(2), Ok(Value::U8(3))),
            (Value::I16(-1), Value::I16(2), Ok(Value::I16(1))),
            (Value::U16(1), Value::U16(2), Ok(Value::U16(3))),
            (Value::I32(-1), Value::I32(2), Ok(Value::I32(1))),
            (Value::U32(1), Value::U32(2), Ok(Value::U32(3))),
            (Value::I64(-1), Value::I64(2), Ok(Value::I64(1))),
            (Value::U64(1), Value::U64(2), Ok(Value::U64(3))),
            (Value::F32(-1.), Value::F32(2.), Ok(Value::F32(1.))),
            (Value::F64(-1.), Value::F64(2.), Ok(Value::F64(1.))),
            (Value::Generic(1), Value::U32(2), Err(Error::TypeMismatch)),
        ] {
            assert_eq!(v1.add(v2, addr_mask), result);
        }
    }

    #[test]
    #[rustfmt::skip]
    fn value_sub() {
        let addr_mask = 0xffff_ffff;
        for &(v1, v2, result) in &[
            (Value::Generic(3), Value::Generic(2), Ok(Value::Generic(1))),
            (Value::I8(-1), Value::I8(2), Ok(Value::I8(-3))),
            (Value::U8(3), Value::U8(2), Ok(Value::U8(1))),
            (Value::I16(-1), Value::I16(2), Ok(Value::I16(-3))),
            (Value::U16(3), Value::U16(2), Ok(Value::U16(1))),
            (Value::I32(-1), Value::I32(2), Ok(Value::I32(-3))),
            (Value::U32(3), Value::U32(2), Ok(Value::U32(1))),
            (Value::I64(-1), Value::I64(2), Ok(Value::I64(-3))),
            (Value::U64(3), Value::U64(2), Ok(Value::U64(1))),
            (Value::F32(-1.), Value::F32(2.), Ok(Value::F32(-3.))),
            (Value::F64(-1.), Value::F64(2.), Ok(Value::F64(-3.))),
            (Value::Generic(3), Value::U32(2), Err(Error::TypeMismatch)),
        ] {
            assert_eq!(v1.sub(v2, addr_mask), result);
        }
    }

    #[test]
    #[rustfmt::skip]
    fn value_mul() {
        let addr_mask = 0xffff_ffff;
        for &(v1, v2, result) in &[
            (Value::Generic(2), Value::Generic(3), Ok(Value::Generic(6))),
            (Value::I8(-2), Value::I8(3), Ok(Value::I8(-6))),
            (Value::U8(2), Value::U8(3), Ok(Value::U8(6))),
            (Value::I16(-2), Value::I16(3), Ok(Value::I16(-6))),
            (Value::U16(2), Value::U16(3), Ok(Value::U16(6))),
            (Value::I32(-2), Value::I32(3), Ok(Value::I32(-6))),
            (Value::U32(2), Value::U32(3), Ok(Value::U32(6))),
            (Value::I64(-2), Value::I64(3), Ok(Value::I64(-6))),
            (Value::U64(2), Value::U64(3), Ok(Value::U64(6))),
            (Value::F32(-2.), Value::F32(3.), Ok(Value::F32(-6.))),
            (Value::F64(-2.), Value::F64(3.), Ok(Value::F64(-6.))),
            (Value::Generic(2), Value::U32(3), Err(Error::TypeMismatch)),
        ] {
            assert_eq!(v1.mul(v2, addr_mask), result);
        }
    }

    #[test]
    #[rustfmt::skip]
    fn value_div() {
        let addr_mask = 0xffff_ffff;
        for &(v1, v2, result) in &[
            (Value::Generic(6), Value::Generic(3), Ok(Value::Generic(2))),
            (Value::I8(-6), Value::I8(3), Ok(Value::I8(-2))),
            (Value::U8(6), Value::U8(3), Ok(Value::U8(2))),
            (Value::I16(-6), Value::I16(3), Ok(Value::I16(-2))),
            (Value::U16(6), Value::U16(3), Ok(Value::U16(2))),
            (Value::I32(-6), Value::I32(3), Ok(Value::I32(-2))),
            (Value::U32(6), Value::U32(3), Ok(Value::U32(2))),
            (Value::I64(-6), Value::I64(3), Ok(Value::I64(-2))),
            (Value::U64(6), Value::U64(3), Ok(Value::U64(2))),
            (Value::F32(-6.), Value::F32(3.), Ok(Value::F32(-2.))),
            (Value::F64(-6.), Value::F64(3.), Ok(Value::F64(-2.))),
            (Value::Generic(6), Value::U32(3), Err(Error::TypeMismatch)),
        ] {
            assert_eq!(v1.div(v2, addr_mask), result);
        }
        for &(v1, v2, result) in &[
            (Value::Generic(6), Value::Generic(0), Err(Error::DivisionByZero)),
            (Value::I8(-6), Value::I8(0), Err(Error::DivisionByZero)),
            (Value::U8(6), Value::U8(0), Err(Error::DivisionByZero)),
            (Value::I16(-6), Value::I16(0), Err(Error::DivisionByZero)),
            (Value::U16(6), Value::U16(0), Err(Error::DivisionByZero)),
            (Value::I32(-6), Value::I32(0), Err(Error::DivisionByZero)),
            (Value::U32(6), Value::U32(0), Err(Error::DivisionByZero)),
            (Value::I64(-6), Value::I64(0), Err(Error::DivisionByZero)),
            (Value::U64(6), Value::U64(0), Err(Error::DivisionByZero)),
            (Value::F32(-6.), Value::F32(0.), Ok(Value::F32(-6. / 0.))),
            (Value::F64(-6.), Value::F64(0.), Ok(Value::F64(-6. / 0.))),
        ] {
            assert_eq!(v1.div(v2, addr_mask), result);
        }
    }

    #[test]
    #[rustfmt::skip]
    fn value_rem() {
        let addr_mask = 0xffff_ffff;
        for &(v1, v2, result) in &[
            (Value::Generic(3), Value::Generic(2), Ok(Value::Generic(1))),
            (Value::I8(-3), Value::I8(2), Ok(Value::I8(-1))),
            (Value::U8(3), Value::U8(2), Ok(Value::U8(1))),
            (Value::I16(-3), Value::I16(2), Ok(Value::I16(-1))),
            (Value::U16(3), Value::U16(2), Ok(Value::U16(1))),
            (Value::I32(-3), Value::I32(2), Ok(Value::I32(-1))),
            (Value::U32(3), Value::U32(2), Ok(Value::U32(1))),
            (Value::I64(-3), Value::I64(2), Ok(Value::I64(-1))),
            (Value::U64(3), Value::U64(2), Ok(Value::U64(1))),
            (Value::F32(-3.), Value::F32(2.), Err(Error::IntegralTypeRequired)),
            (Value::F64(-3.), Value::F64(2.), Err(Error::IntegralTypeRequired)),
            (Value::Generic(3), Value::U32(2), Err(Error::TypeMismatch)),
        ] {
            assert_eq!(v1.rem(v2, addr_mask), result);
        }
        for &(v1, v2, result) in &[
            (Value::Generic(3), Value::Generic(0), Err(Error::DivisionByZero)),
            (Value::I8(-3), Value::I8(0), Err(Error::DivisionByZero)),
            (Value::U8(3), Value::U8(0), Err(Error::DivisionByZero)),
            (Value::I16(-3), Value::I16(0), Err(Error::DivisionByZero)),
            (Value::U16(3), Value::U16(0), Err(Error::DivisionByZero)),
            (Value::I32(-3), Value::I32(0), Err(Error::DivisionByZero)),
            (Value::U32(3), Value::U32(0), Err(Error::DivisionByZero)),
            (Value::I64(-3), Value::I64(0), Err(Error::DivisionByZero)),
            (Value::U64(3), Value::U64(0), Err(Error::DivisionByZero)),
        ] {
            assert_eq!(v1.rem(v2, addr_mask), result);
        }
    }

    #[test]
    #[rustfmt::skip]
    fn value_not() {
        let addr_mask = 0xffff_ffff;
        for &(v, result) in &[
            (Value::Generic(1), Ok(Value::Generic(!1))),
            (Value::I8(1), Ok(Value::I8(!1))),
            (Value::U8(1), Ok(Value::U8(!1))),
            (Value::I16(1), Ok(Value::I16(!1))),
            (Value::U16(1), Ok(Value::U16(!1))),
            (Value::I32(1), Ok(Value::I32(!1))),
            (Value::U32(1), Ok(Value::U32(!1))),
            (Value::I64(1), Ok(Value::I64(!1))),
            (Value::U64(1), Ok(Value::U64(!1))),
            (Value::F32(1.), Err(Error::IntegralTypeRequired)),
            (Value::F64(1.), Err(Error::IntegralTypeRequired)),
        ] {
            assert_eq!(v.not(addr_mask), result);
        }
    }

    #[test]
    #[rustfmt::skip]
    fn value_and() {
        let addr_mask = 0xffff_ffff;
        for &(v1, v2, result) in &[
            (Value::Generic(3), Value::Generic(5), Ok(Value::Generic(1))),
            (Value::I8(3), Value::I8(5), Ok(Value::I8(1))),
            (Value::U8(3), Value::U8(5), Ok(Value::U8(1))),
            (Value::I16(3), Value::I16(5), Ok(Value::I16(1))),
            (Value::U16(3), Value::U16(5), Ok(Value::U16(1))),
            (Value::I32(3), Value::I32(5), Ok(Value::I32(1))),
            (Value::U32(3), Value::U32(5), Ok(Value::U32(1))),
            (Value::I64(3), Value::I64(5), Ok(Value::I64(1))),
            (Value::U64(3), Value::U64(5), Ok(Value::U64(1))),
            (Value::F32(3.), Value::F32(5.), Err(Error::IntegralTypeRequired)),
            (Value::F64(3.), Value::F64(5.), Err(Error::IntegralTypeRequired)),
            (Value::Generic(3), Value::U32(5), Err(Error::TypeMismatch)),
        ] {
            assert_eq!(v1.and(v2, addr_mask), result);
        }
    }

    #[test]
    #[rustfmt::skip]
    fn value_or() {
        let addr_mask = 0xffff_ffff;
        for &(v1, v2, result) in &[
            (Value::Generic(3), Value::Generic(5), Ok(Value::Generic(7))),
            (Value::I8(3), Value::I8(5), Ok(Value::I8(7))),
            (Value::U8(3), Value::U8(5), Ok(Value::U8(7))),
            (Value::I16(3), Value::I16(5), Ok(Value::I16(7))),
            (Value::U16(3), Value::U16(5), Ok(Value::U16(7))),
            (Value::I32(3), Value::I32(5), Ok(Value::I32(7))),
            (Value::U32(3), Value::U32(5), Ok(Value::U32(7))),
            (Value::I64(3), Value::I64(5), Ok(Value::I64(7))),
            (Value::U64(3), Value::U64(5), Ok(Value::U64(7))),
            (Value::F32(3.), Value::F32(5.), Err(Error::IntegralTypeRequired)),
            (Value::F64(3.), Value::F64(5.), Err(Error::IntegralTypeRequired)),
            (Value::Generic(3), Value::U32(5), Err(Error::TypeMismatch)),
        ] {
            assert_eq!(v1.or(v2, addr_mask), result);
        }
    }

    #[test]
    #[rustfmt::skip]
    fn value_xor() {
        let addr_mask = 0xffff_ffff;
        for &(v1, v2, result) in &[
            (Value::Generic(3), Value::Generic(5), Ok(Value::Generic(6))),
            (Value::I8(3), Value::I8(5), Ok(Value::I8(6))),
            (Value::U8(3), Value::U8(5), Ok(Value::U8(6))),
            (Value::I16(3), Value::I16(5), Ok(Value::I16(6))),
            (Value::U16(3), Value::U16(5), Ok(Value::U16(6))),
            (Value::I32(3), Value::I32(5), Ok(Value::I32(6))),
            (Value::U32(3), Value::U32(5), Ok(Value::U32(6))),
            (Value::I64(3), Value::I64(5), Ok(Value::I64(6))),
            (Value::U64(3), Value::U64(5), Ok(Value::U64(6))),
            (Value::F32(3.), Value::F32(5.), Err(Error::IntegralTypeRequired)),
            (Value::F64(3.), Value::F64(5.), Err(Error::IntegralTypeRequired)),
            (Value::Generic(3), Value::U32(5), Err(Error::TypeMismatch)),
        ] {
            assert_eq!(v1.xor(v2, addr_mask), result);
        }
    }

    #[test]
    #[rustfmt::skip]
    fn value_shl() {
        let addr_mask = 0xffff_ffff;
        for &(v1, v2, result) in &[
            // One of each type
            (Value::Generic(3), Value::Generic(5), Ok(Value::Generic(96))),
            (Value::I8(3), Value::U8(5), Ok(Value::I8(96))),
            (Value::U8(3), Value::I8(5), Ok(Value::U8(96))),
            (Value::I16(3), Value::U16(5), Ok(Value::I16(96))),
            (Value::U16(3), Value::I16(5), Ok(Value::U16(96))),
            (Value::I32(3), Value::U32(5), Ok(Value::I32(96))),
            (Value::U32(3), Value::I32(5), Ok(Value::U32(96))),
            (Value::I64(3), Value::U64(5), Ok(Value::I64(96))),
            (Value::U64(3), Value::I64(5), Ok(Value::U64(96))),
            (Value::F32(3.), Value::U8(5), Err(Error::IntegralTypeRequired)),
            (Value::F64(3.), Value::U8(5), Err(Error::IntegralTypeRequired)),
            // Invalid shifts
            (Value::U8(3), Value::I8(-5), Err(Error::InvalidShiftExpression)),
            (Value::U8(3), Value::I16(-5), Err(Error::InvalidShiftExpression)),
            (Value::U8(3), Value::I32(-5), Err(Error::InvalidShiftExpression)),
            (Value::U8(3), Value::I64(-5), Err(Error::InvalidShiftExpression)),
            (Value::U8(3), Value::F32(5.), Err(Error::InvalidShiftExpression)),
            (Value::U8(3), Value::F64(5.), Err(Error::InvalidShiftExpression)),
            // Large shifts
            (Value::Generic(3), Value::Generic(32), Ok(Value::Generic(0))),
            (Value::I8(3), Value::U8(8), Ok(Value::I8(0))),
            (Value::U8(3), Value::I8(9), Ok(Value::U8(0))),
            (Value::I16(3), Value::U16(17), Ok(Value::I16(0))),
            (Value::U16(3), Value::I16(16), Ok(Value::U16(0))),
            (Value::I32(3), Value::U32(32), Ok(Value::I32(0))),
            (Value::U32(3), Value::I32(33), Ok(Value::U32(0))),
            (Value::I64(3), Value::U64(65), Ok(Value::I64(0))),
            (Value::U64(3), Value::I64(64), Ok(Value::U64(0))),
        ] {
            assert_eq!(v1.shl(v2, addr_mask), result);
        }
    }

    #[test]
    #[rustfmt::skip]
    fn value_shr() {
        let addr_mask = 0xffff_ffff;
        for &(v1, v2, result) in &[
            // One of each type
            (Value::Generic(96), Value::Generic(5), Ok(Value::Generic(3))),
            (Value::I8(96), Value::U8(5), Err(Error::UnsupportedTypeOperation)),
            (Value::U8(96), Value::I8(5), Ok(Value::U8(3))),
            (Value::I16(96), Value::U16(5), Err(Error::UnsupportedTypeOperation)),
            (Value::U16(96), Value::I16(5), Ok(Value::U16(3))),
            (Value::I32(96), Value::U32(5), Err(Error::UnsupportedTypeOperation)),
            (Value::U32(96), Value::I32(5), Ok(Value::U32(3))),
            (Value::I64(96), Value::U64(5), Err(Error::UnsupportedTypeOperation)),
            (Value::U64(96), Value::I64(5), Ok(Value::U64(3))),
            (Value::F32(96.), Value::U8(5), Err(Error::IntegralTypeRequired)),
            (Value::F64(96.), Value::U8(5), Err(Error::IntegralTypeRequired)),
            // Invalid shifts
            (Value::U8(96), Value::I8(-5), Err(Error::InvalidShiftExpression)),
            (Value::U8(96), Value::I16(-5), Err(Error::InvalidShiftExpression)),
            (Value::U8(96), Value::I32(-5), Err(Error::InvalidShiftExpression)),
            (Value::U8(96), Value::I64(-5), Err(Error::InvalidShiftExpression)),
            (Value::U8(96), Value::F32(5.), Err(Error::InvalidShiftExpression)),
            (Value::U8(96), Value::F64(5.), Err(Error::InvalidShiftExpression)),
            // Large shifts
            (Value::Generic(96), Value::Generic(32), Ok(Value::Generic(0))),
            (Value::U8(96), Value::I8(9), Ok(Value::U8(0))),
            (Value::U16(96), Value::I16(16), Ok(Value::U16(0))),
            (Value::U32(96), Value::I32(33), Ok(Value::U32(0))),
            (Value::U64(96), Value::I64(64), Ok(Value::U64(0))),
        ] {
            assert_eq!(v1.shr(v2, addr_mask), result);
        }
    }

    #[test]
    #[rustfmt::skip]
    fn value_shra() {
        let addr_mask = 0xffff_ffff;
        for &(v1, v2, result) in &[
            // One of each type
            (Value::Generic(u64::from(-96i32 as u32)), Value::Generic(5), Ok(Value::Generic(-3i64 as u64))),
            (Value::I8(-96), Value::U8(5), Ok(Value::I8(-3))),
            (Value::U8(96), Value::I8(5), Err(Error::UnsupportedTypeOperation)),
            (Value::I16(-96), Value::U16(5), Ok(Value::I16(-3))),
            (Value::U16(96), Value::I16(5), Err(Error::UnsupportedTypeOperation)),
            (Value::I32(-96), Value::U32(5), Ok(Value::I32(-3))),
            (Value::U32(96), Value::I32(5), Err(Error::UnsupportedTypeOperation)),
            (Value::I64(-96), Value::U64(5), Ok(Value::I64(-3))),
            (Value::U64(96), Value::I64(5), Err(Error::UnsupportedTypeOperation)),
            (Value::F32(96.), Value::U8(5), Err(Error::IntegralTypeRequired)),
            (Value::F64(96.), Value::U8(5), Err(Error::IntegralTypeRequired)),
            // Invalid shifts
            (Value::U8(96), Value::I8(-5), Err(Error::InvalidShiftExpression)),
            (Value::U8(96), Value::I16(-5), Err(Error::InvalidShiftExpression)),
            (Value::U8(96), Value::I32(-5), Err(Error::InvalidShiftExpression)),
            (Value::U8(96), Value::I64(-5), Err(Error::InvalidShiftExpression)),
            (Value::U8(96), Value::F32(5.), Err(Error::InvalidShiftExpression)),
            (Value::U8(96), Value::F64(5.), Err(Error::InvalidShiftExpression)),
            // Large shifts
            (Value::Generic(96), Value::Generic(32), Ok(Value::Generic(0))),
            (Value::I8(96), Value::U8(8), Ok(Value::I8(0))),
            (Value::I8(-96), Value::U8(8), Ok(Value::I8(-1))),
            (Value::I16(96), Value::U16(17), Ok(Value::I16(0))),
            (Value::I16(-96), Value::U16(17), Ok(Value::I16(-1))),
            (Value::I32(96), Value::U32(32), Ok(Value::I32(0))),
            (Value::I32(-96), Value::U32(32), Ok(Value::I32(-1))),
            (Value::I64(96), Value::U64(65), Ok(Value::I64(0))),
            (Value::I64(-96), Value::U64(65), Ok(Value::I64(-1))),
        ] {
            assert_eq!(v1.shra(v2, addr_mask), result);
        }
    }

    #[test]
    fn value_eq() {
        let addr_mask = 0xffff_ffff;
        for &(v1, v2, result) in &[
            (Value::Generic(3), Value::Generic(3), Ok(Value::Generic(1))),
            (Value::Generic(!3), Value::Generic(3), Ok(Value::Generic(0))),
            (Value::I8(3), Value::I8(3), Ok(Value::Generic(1))),
            (Value::I8(!3), Value::I8(3), Ok(Value::Generic(0))),
            (Value::U8(3), Value::U8(3), Ok(Value::Generic(1))),
            (Value::U8(!3), Value::U8(3), Ok(Value::Generic(0))),
            (Value::I16(3), Value::I16(3), Ok(Value::Generic(1))),
            (Value::I16(!3), Value::I16(3), Ok(Value::Generic(0))),
            (Value::U16(3), Value::U16(3), Ok(Value::Generic(1))),
            (Value::U16(!3), Value::U16(3), Ok(Value::Generic(0))),
            (Value::I32(3), Value::I32(3), Ok(Value::Generic(1))),
            (Value::I32(!3), Value::I32(3), Ok(Value::Generic(0))),
            (Value::U32(3), Value::U32(3), Ok(Value::Generic(1))),
            (Value::U32(!3), Value::U32(3), Ok(Value::Generic(0))),
            (Value::I64(3), Value::I64(3), Ok(Value::Generic(1))),
            (Value::I64(!3), Value::I64(3), Ok(Value::Generic(0))),
            (Value::U64(3), Value::U64(3), Ok(Value::Generic(1))),
            (Value::U64(!3), Value::U64(3), Ok(Value::Generic(0))),
            (Value::F32(3.), Value::F32(3.), Ok(Value::Generic(1))),
            (Value::F32(-3.), Value::F32(3.), Ok(Value::Generic(0))),
            (Value::F64(3.), Value::F64(3.), Ok(Value::Generic(1))),
            (Value::F64(-3.), Value::F64(3.), Ok(Value::Generic(0))),
            (Value::Generic(3), Value::U32(3), Err(Error::TypeMismatch)),
        ] {
            assert_eq!(v1.eq(v2, addr_mask), result);
        }
    }

    #[test]
    fn value_ne() {
        let addr_mask = 0xffff_ffff;
        for &(v1, v2, result) in &[
            (Value::Generic(3), Value::Generic(3), Ok(Value::Generic(0))),
            (Value::Generic(!3), Value::Generic(3), Ok(Value::Generic(1))),
            (Value::I8(3), Value::I8(3), Ok(Value::Generic(0))),
            (Value::I8(!3), Value::I8(3), Ok(Value::Generic(1))),
            (Value::U8(3), Value::U8(3), Ok(Value::Generic(0))),
            (Value::U8(!3), Value::U8(3), Ok(Value::Generic(1))),
            (Value::I16(3), Value::I16(3), Ok(Value::Generic(0))),
            (Value::I16(!3), Value::I16(3), Ok(Value::Generic(1))),
            (Value::U16(3), Value::U16(3), Ok(Value::Generic(0))),
            (Value::U16(!3), Value::U16(3), Ok(Value::Generic(1))),
            (Value::I32(3), Value::I32(3), Ok(Value::Generic(0))),
            (Value::I32(!3), Value::I32(3), Ok(Value::Generic(1))),
            (Value::U32(3), Value::U32(3), Ok(Value::Generic(0))),
            (Value::U32(!3), Value::U32(3), Ok(Value::Generic(1))),
            (Value::I64(3), Value::I64(3), Ok(Value::Generic(0))),
            (Value::I64(!3), Value::I64(3), Ok(Value::Generic(1))),
            (Value::U64(3), Value::U64(3), Ok(Value::Generic(0))),
            (Value::U64(!3), Value::U64(3), Ok(Value::Generic(1))),
            (Value::F32(3.), Value::F32(3.), Ok(Value::Generic(0))),
            (Value::F32(-3.), Value::F32(3.), Ok(Value::Generic(1))),
            (Value::F64(3.), Value::F64(3.), Ok(Value::Generic(0))),
            (Value::F64(-3.), Value::F64(3.), Ok(Value::Generic(1))),
            (Value::Generic(3), Value::U32(3), Err(Error::TypeMismatch)),
        ] {
            assert_eq!(v1.ne(v2, addr_mask), result);
        }
    }

    #[test]
    fn value_ge() {
        let addr_mask = 0xffff_ffff;
        for &(v1, v2, result) in &[
            (Value::Generic(3), Value::Generic(!3), Ok(Value::Generic(1))),
            (Value::Generic(!3), Value::Generic(3), Ok(Value::Generic(0))),
            (Value::I8(3), Value::I8(!3), Ok(Value::Generic(1))),
            (Value::I8(!3), Value::I8(3), Ok(Value::Generic(0))),
            (Value::U8(3), Value::U8(!3), Ok(Value::Generic(0))),
            (Value::U8(!3), Value::U8(3), Ok(Value::Generic(1))),
            (Value::I16(3), Value::I16(!3), Ok(Value::Generic(1))),
            (Value::I16(!3), Value::I16(3), Ok(Value::Generic(0))),
            (Value::U16(3), Value::U16(!3), Ok(Value::Generic(0))),
            (Value::U16(!3), Value::U16(3), Ok(Value::Generic(1))),
            (Value::I32(3), Value::I32(!3), Ok(Value::Generic(1))),
            (Value::I32(!3), Value::I32(3), Ok(Value::Generic(0))),
            (Value::U32(3), Value::U32(!3), Ok(Value::Generic(0))),
            (Value::U32(!3), Value::U32(3), Ok(Value::Generic(1))),
            (Value::I64(3), Value::I64(!3), Ok(Value::Generic(1))),
            (Value::I64(!3), Value::I64(3), Ok(Value::Generic(0))),
            (Value::U64(3), Value::U64(!3), Ok(Value::Generic(0))),
            (Value::U64(!3), Value::U64(3), Ok(Value::Generic(1))),
            (Value::F32(3.), Value::F32(-3.), Ok(Value::Generic(1))),
            (Value::F32(-3.), Value::F32(3.), Ok(Value::Generic(0))),
            (Value::F64(3.), Value::F64(-3.), Ok(Value::Generic(1))),
            (Value::F64(-3.), Value::F64(3.), Ok(Value::Generic(0))),
            (Value::Generic(3), Value::U32(3), Err(Error::TypeMismatch)),
        ] {
            assert_eq!(v1.ge(v2, addr_mask), result);
        }
    }

    #[test]
    fn value_gt() {
        let addr_mask = 0xffff_ffff;
        for &(v1, v2, result) in &[
            (Value::Generic(3), Value::Generic(!3), Ok(Value::Generic(1))),
            (Value::Generic(!3), Value::Generic(3), Ok(Value::Generic(0))),
            (Value::I8(3), Value::I8(!3), Ok(Value::Generic(1))),
            (Value::I8(!3), Value::I8(3), Ok(Value::Generic(0))),
            (Value::U8(3), Value::U8(!3), Ok(Value::Generic(0))),
            (Value::U8(!3), Value::U8(3), Ok(Value::Generic(1))),
            (Value::I16(3), Value::I16(!3), Ok(Value::Generic(1))),
            (Value::I16(!3), Value::I16(3), Ok(Value::Generic(0))),
            (Value::U16(3), Value::U16(!3), Ok(Value::Generic(0))),
            (Value::U16(!3), Value::U16(3), Ok(Value::Generic(1))),
            (Value::I32(3), Value::I32(!3), Ok(Value::Generic(1))),
            (Value::I32(!3), Value::I32(3), Ok(Value::Generic(0))),
            (Value::U32(3), Value::U32(!3), Ok(Value::Generic(0))),
            (Value::U32(!3), Value::U32(3), Ok(Value::Generic(1))),
            (Value::I64(3), Value::I64(!3), Ok(Value::Generic(1))),
            (Value::I64(!3), Value::I64(3), Ok(Value::Generic(0))),
            (Value::U64(3), Value::U64(!3), Ok(Value::Generic(0))),
            (Value::U64(!3), Value::U64(3), Ok(Value::Generic(1))),
            (Value::F32(3.), Value::F32(-3.), Ok(Value::Generic(1))),
            (Value::F32(-3.), Value::F32(3.), Ok(Value::Generic(0))),
            (Value::F64(3.), Value::F64(-3.), Ok(Value::Generic(1))),
            (Value::F64(-3.), Value::F64(3.), Ok(Value::Generic(0))),
            (Value::Generic(3), Value::U32(3), Err(Error::TypeMismatch)),
        ] {
            assert_eq!(v1.gt(v2, addr_mask), result);
        }
    }

    #[test]
    fn value_le() {
        let addr_mask = 0xffff_ffff;
        for &(v1, v2, result) in &[
            (Value::Generic(3), Value::Generic(!3), Ok(Value::Generic(0))),
            (Value::Generic(!3), Value::Generic(3), Ok(Value::Generic(1))),
            (Value::I8(3), Value::I8(!3), Ok(Value::Generic(0))),
            (Value::I8(!3), Value::I8(3), Ok(Value::Generic(1))),
            (Value::U8(3), Value::U8(!3), Ok(Value::Generic(1))),
            (Value::U8(!3), Value::U8(3), Ok(Value::Generic(0))),
            (Value::I16(3), Value::I16(!3), Ok(Value::Generic(0))),
            (Value::I16(!3), Value::I16(3), Ok(Value::Generic(1))),
            (Value::U16(3), Value::U16(!3), Ok(Value::Generic(1))),
            (Value::U16(!3), Value::U16(3), Ok(Value::Generic(0))),
            (Value::I32(3), Value::I32(!3), Ok(Value::Generic(0))),
            (Value::I32(!3), Value::I32(3), Ok(Value::Generic(1))),
            (Value::U32(3), Value::U32(!3), Ok(Value::Generic(1))),
            (Value::U32(!3), Value::U32(3), Ok(Value::Generic(0))),
            (Value::I64(3), Value::I64(!3), Ok(Value::Generic(0))),
            (Value::I64(!3), Value::I64(3), Ok(Value::Generic(1))),
            (Value::U64(3), Value::U64(!3), Ok(Value::Generic(1))),
            (Value::U64(!3), Value::U64(3), Ok(Value::Generic(0))),
            (Value::F32(3.), Value::F32(-3.), Ok(Value::Generic(0))),
            (Value::F32(-3.), Value::F32(3.), Ok(Value::Generic(1))),
            (Value::F64(3.), Value::F64(-3.), Ok(Value::Generic(0))),
            (Value::F64(-3.), Value::F64(3.), Ok(Value::Generic(1))),
            (Value::Generic(3), Value::U32(3), Err(Error::TypeMismatch)),
        ] {
            assert_eq!(v1.le(v2, addr_mask), result);
        }
    }

    #[test]
    fn value_lt() {
        let addr_mask = 0xffff_ffff;
        for &(v1, v2, result) in &[
            (Value::Generic(3), Value::Generic(!3), Ok(Value::Generic(0))),
            (Value::Generic(!3), Value::Generic(3), Ok(Value::Generic(1))),
            (Value::I8(3), Value::I8(!3), Ok(Value::Generic(0))),
            (Value::I8(!3), Value::I8(3), Ok(Value::Generic(1))),
            (Value::U8(3), Value::U8(!3), Ok(Value::Generic(1))),
            (Value::U8(!3), Value::U8(3), Ok(Value::Generic(0))),
            (Value::I16(3), Value::I16(!3), Ok(Value::Generic(0))),
            (Value::I16(!3), Value::I16(3), Ok(Value::Generic(1))),
            (Value::U16(3), Value::U16(!3), Ok(Value::Generic(1))),
            (Value::U16(!3), Value::U16(3), Ok(Value::Generic(0))),
            (Value::I32(3), Value::I32(!3), Ok(Value::Generic(0))),
            (Value::I32(!3), Value::I32(3), Ok(Value::Generic(1))),
            (Value::U32(3), Value::U32(!3), Ok(Value::Generic(1))),
            (Value::U32(!3), Value::U32(3), Ok(Value::Generic(0))),
            (Value::I64(3), Value::I64(!3), Ok(Value::Generic(0))),
            (Value::I64(!3), Value::I64(3), Ok(Value::Generic(1))),
            (Value::U64(3), Value::U64(!3), Ok(Value::Generic(1))),
            (Value::U64(!3), Value::U64(3), Ok(Value::Generic(0))),
            (Value::F32(3.), Value::F32(-3.), Ok(Value::Generic(0))),
            (Value::F32(-3.), Value::F32(3.), Ok(Value::Generic(1))),
            (Value::F64(3.), Value::F64(-3.), Ok(Value::Generic(0))),
            (Value::F64(-3.), Value::F64(3.), Ok(Value::Generic(1))),
            (Value::Generic(3), Value::U32(3), Err(Error::TypeMismatch)),
        ] {
            assert_eq!(v1.lt(v2, addr_mask), result);
        }
    }
}
