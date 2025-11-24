use std::{fmt, iter::FromIterator};

use crate::ast;

/// Get the "shape" of a fields container, such as a struct or variant.
pub trait AsShape {
    /// Get the "shape" of a fields container.
    fn as_shape(&self) -> Shape;
}

impl<T> AsShape for ast::Fields<T> {
    fn as_shape(&self) -> Shape {
        match self.style {
            ast::Style::Tuple if self.fields.len() == 1 => Shape::Newtype,
            ast::Style::Tuple => Shape::Tuple,
            ast::Style::Struct => Shape::Named,
            ast::Style::Unit => Shape::Unit,
        }
    }
}

impl AsShape for syn::Fields {
    fn as_shape(&self) -> Shape {
        match self {
            syn::Fields::Named(fields) => fields.as_shape(),
            syn::Fields::Unnamed(fields) => fields.as_shape(),
            syn::Fields::Unit => Shape::Unit,
        }
    }
}

impl AsShape for syn::FieldsNamed {
    fn as_shape(&self) -> Shape {
        Shape::Named
    }
}

impl AsShape for syn::FieldsUnnamed {
    fn as_shape(&self) -> Shape {
        if self.unnamed.len() == 1 {
            Shape::Newtype
        } else {
            Shape::Tuple
        }
    }
}

impl AsShape for syn::DataStruct {
    fn as_shape(&self) -> Shape {
        self.fields.as_shape()
    }
}

impl AsShape for syn::Variant {
    fn as_shape(&self) -> Shape {
        self.fields.as_shape()
    }
}

/// Description of how fields in a struct or variant are syntactically laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// A set of named fields, e.g. `{ field: String }`.
    Named,
    /// A list of unnamed fields, e.g. `(String, u64)`.
    Tuple,
    /// No fields, e.g. `struct Example;`
    Unit,
    /// A special case of [`Tuple`](Shape#variant.Tuple) with exactly one field, e.g. `(String)`.
    Newtype,
}

impl Shape {
    pub fn description(&self) -> &'static str {
        match self {
            Shape::Named => "named fields",
            Shape::Tuple => "unnamed fields",
            Shape::Unit => "no fields",
            Shape::Newtype => "one unnamed field",
        }
    }
}

impl fmt::Display for Shape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl AsShape for Shape {
    fn as_shape(&self) -> Shape {
        *self
    }
}

/// A set of [`Shape`] values, which correctly handles the relationship between
/// [newtype](Shape#variant.Newtype) and [tuple](Shape#variant.Tuple) shapes.
///
/// # Example
/// ```rust
/// # use darling_core::util::{Shape, ShapeSet};
/// let shape_set = ShapeSet::new(vec![Shape::Tuple]);
///
/// // This is correct, because all newtypes are single-field tuples.
/// assert!(shape_set.contains(&Shape::Newtype));
/// ```
#[derive(Debug, Clone, Default)]
pub struct ShapeSet {
    newtype: bool,
    named: bool,
    tuple: bool,
    unit: bool,
}

impl ShapeSet {
    /// Create a new `ShapeSet` which includes the specified items.
    ///
    /// # Exampe
    /// ```rust
    /// # use darling_core::util::{Shape, ShapeSet};
    /// let shape_set = ShapeSet::new(vec![Shape::Named, Shape::Newtype]);
    /// assert!(shape_set.contains(&Shape::Newtype));
    /// ```
    pub fn new(items: impl IntoIterator<Item = Shape>) -> Self {
        items.into_iter().collect()
    }

    /// Insert all possible shapes into the set.
    ///
    /// This is equivalent to calling [`insert`](ShapeSet#method.insert) with every value of [`Shape`].
    ///
    /// # Example
    /// ```rust
    /// # use darling_core::util::{Shape, ShapeSet};
    /// let mut shape_set = ShapeSet::default();
    /// shape_set.insert_all();
    /// assert!(shape_set.contains(&Shape::Named));
    /// ```
    pub fn insert_all(&mut self) {
        self.insert(Shape::Named);
        self.insert(Shape::Newtype);
        self.insert(Shape::Tuple);
        self.insert(Shape::Unit);
    }

    /// Insert a shape into the set, so that the set will match that shape
    pub fn insert(&mut self, shape: Shape) {
        match shape {
            Shape::Named => self.named = true,
            Shape::Tuple => self.tuple = true,
            Shape::Unit => self.unit = true,
            Shape::Newtype => self.newtype = true,
        }
    }

    /// Whether this set is empty.
    pub fn is_empty(&self) -> bool {
        !self.named && !self.newtype && !self.tuple && !self.unit
    }

    fn contains_shape(&self, shape: Shape) -> bool {
        match shape {
            Shape::Named => self.named,
            Shape::Tuple => self.tuple,
            Shape::Unit => self.unit,
            Shape::Newtype => self.newtype || self.tuple,
        }
    }

    /// Check if a fields container's shape is in this set.
    pub fn contains(&self, fields: &impl AsShape) -> bool {
        self.contains_shape(fields.as_shape())
    }

    /// Check if a field container's shape is in this set of shapes, and produce
    /// an [`Error`](crate::Error) if it does not.
    pub fn check(&self, fields: &impl AsShape) -> crate::Result<()> {
        let shape = fields.as_shape();

        if self.contains_shape(shape) {
            Ok(())
        } else {
            Err(crate::Error::unsupported_shape_with_expected(
                shape.description(),
                self,
            ))
        }
    }

    fn to_vec(&self) -> Vec<Shape> {
        let mut shapes = Vec::with_capacity(3);

        if self.named {
            shapes.push(Shape::Named);
        }

        if self.tuple || self.newtype {
            shapes.push(if self.tuple {
                Shape::Tuple
            } else {
                Shape::Newtype
            });
        }

        if self.unit {
            shapes.push(Shape::Unit)
        }

        shapes
    }
}

impl fmt::Display for ShapeSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let shapes = self.to_vec();

        match shapes.len() {
            0 => write!(f, "nothing"),
            1 => write!(f, "{}", shapes[0]),
            2 => write!(f, "{} or {}", shapes[0], shapes[1]),
            3 => write!(f, "{}, {}, or {}", shapes[0], shapes[1], shapes[2]),
            _ => unreachable!(),
        }
    }
}

impl FromIterator<Shape> for ShapeSet {
    fn from_iter<T: IntoIterator<Item = Shape>>(iter: T) -> Self {
        let mut output = ShapeSet::default();
        for shape in iter.into_iter() {
            output.insert(shape);
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;

    #[test]
    fn any_accepts_anything() {
        let mut filter = ShapeSet::default();
        filter.insert_all();
        let unit_struct: syn::DeriveInput = syn::parse_quote! {
            struct Example;
        };
        if let syn::Data::Struct(data) = unit_struct.data {
            assert!(filter.contains(&data));
        } else {
            panic!("Struct not parsed as struct");
        };
    }

    #[test]
    fn tuple_accepts_newtype() {
        let filter = ShapeSet::new(vec![Shape::Tuple]);
        let newtype_struct: syn::DeriveInput = parse_quote! {
            struct Example(String);
        };

        if let syn::Data::Struct(data) = newtype_struct.data {
            assert!(filter.contains(&data));
        } else {
            panic!("Struct not parsed as struct");
        };
    }

    #[test]
    fn newtype_rejects_tuple() {
        let filter = ShapeSet::new(vec![Shape::Newtype]);
        let tuple_struct: syn::DeriveInput = parse_quote! {
            struct Example(String, u64);
        };

        if let syn::Data::Struct(data) = tuple_struct.data {
            assert!(!filter.contains(&data));
        } else {
            panic!("Struct not parsed as struct");
        };
    }
}
