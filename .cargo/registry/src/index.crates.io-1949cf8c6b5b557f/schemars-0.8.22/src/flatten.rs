use crate::schema::*;
use crate::{Map, Set};

impl Schema {
    /// This function is only public for use by schemars_derive.
    ///
    /// It should not be considered part of the public API.
    #[doc(hidden)]
    pub fn flatten(self, other: Self) -> Schema {
        if is_null_type(&self) {
            return other;
        } else if is_null_type(&other) {
            return self;
        }
        let s1: SchemaObject = self.into();
        let s2: SchemaObject = other.into();
        Schema::Object(s1.merge(s2))
    }
}

pub(crate) trait Merge: Sized {
    fn merge(self, other: Self) -> Self;
}

macro_rules! impl_merge {
    ($ty:ident { merge: $($merge_field:ident)*, or: $($or_field:ident)*, }) => {
        impl Merge for $ty {
            fn merge(self, other: Self) -> Self {
                $ty {
                    $($merge_field: self.$merge_field.merge(other.$merge_field),)*
                    $($or_field: self.$or_field.or(other.$or_field),)*
                }
            }
        }
    };
    ($ty:ident { or: $($or_field:ident)*, }) => {
        impl_merge!( $ty { merge: , or: $($or_field)*, });
    };
}

// For ObjectValidation::additional_properties.
impl Merge for Option<Box<Schema>> {
    fn merge(self, other: Self) -> Self {
        match (self.map(|x| *x), other.map(|x| *x)) {
            // Perfer permissive schemas.
            (Some(Schema::Bool(true)), _) => Some(Box::new(true.into())),
            (_, Some(Schema::Bool(true))) => Some(Box::new(true.into())),
            (None, _) => None,
            (_, None) => None,

            // Merge if we have two non-trivial schemas.
            (Some(Schema::Object(s1)), Some(Schema::Object(s2))) => {
                Some(Box::new(Schema::Object(s1.merge(s2))))
            }

            // Perfer the more permissive schema.
            (Some(s1 @ Schema::Object(_)), Some(Schema::Bool(false))) => Some(Box::new(s1)),
            (Some(Schema::Bool(false)), Some(s2 @ Schema::Object(_))) => Some(Box::new(s2)),

            // Default to the null schema.
            (Some(Schema::Bool(false)), Some(Schema::Bool(false))) => Some(Box::new(false.into())),
        }
    }
}

impl_merge!(SchemaObject {
    merge: extensions instance_type enum_values
        metadata subschemas number string array object,
    or: format const_value reference,
});

impl Merge for Metadata {
    fn merge(self, other: Self) -> Self {
        Metadata {
            id: self.id.or(other.id),
            title: self.title.or(other.title),
            description: self.description.or(other.description),
            default: self.default.or(other.default),
            deprecated: self.deprecated || other.deprecated,
            read_only: self.read_only || other.read_only,
            write_only: self.write_only || other.write_only,
            examples: self.examples.merge(other.examples),
        }
    }
}

impl_merge!(SubschemaValidation {
    or: all_of any_of one_of not if_schema then_schema else_schema,
});

impl_merge!(NumberValidation {
    or: multiple_of maximum exclusive_maximum minimum exclusive_minimum,
});

impl_merge!(StringValidation {
    or: max_length min_length pattern,
});

impl_merge!(ArrayValidation {
    or: items additional_items max_items min_items unique_items contains,
});

impl_merge!(ObjectValidation {
    merge: required properties pattern_properties additional_properties,
    or: max_properties min_properties property_names,
});

impl<T: Merge> Merge for Option<T> {
    fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Some(x), Some(y)) => Some(x.merge(y)),
            (None, y) => y,
            (x, None) => x,
        }
    }
}

impl<T: Merge> Merge for Box<T> {
    fn merge(mut self, other: Self) -> Self {
        *self = (*self).merge(*other);
        self
    }
}

impl<T> Merge for Vec<T> {
    fn merge(mut self, other: Self) -> Self {
        self.extend(other);
        self
    }
}

impl<K, V> Merge for Map<K, V>
where
    K: std::hash::Hash + Eq + Ord,
{
    fn merge(mut self, other: Self) -> Self {
        self.extend(other);
        self
    }
}

impl<T: Ord> Merge for Set<T> {
    fn merge(mut self, other: Self) -> Self {
        self.extend(other);
        self
    }
}

impl Merge for SingleOrVec<InstanceType> {
    fn merge(self, other: Self) -> Self {
        if self == other {
            return self;
        }
        let mut vec = match (self, other) {
            (SingleOrVec::Vec(v1), SingleOrVec::Vec(v2)) => v1.merge(v2),
            (SingleOrVec::Vec(mut v), SingleOrVec::Single(s))
            | (SingleOrVec::Single(s), SingleOrVec::Vec(mut v)) => {
                v.push(*s);
                v
            }
            (SingleOrVec::Single(s1), SingleOrVec::Single(s2)) => vec![*s1, *s2],
        };
        vec.sort();
        vec.dedup();
        SingleOrVec::Vec(vec)
    }
}

fn is_null_type(schema: &Schema) -> bool {
    let s = match schema {
        Schema::Object(s) => s,
        _ => return false,
    };
    let instance_type = match &s.instance_type {
        Some(SingleOrVec::Single(t)) => t,
        _ => return false,
    };

    **instance_type == InstanceType::Null
}
