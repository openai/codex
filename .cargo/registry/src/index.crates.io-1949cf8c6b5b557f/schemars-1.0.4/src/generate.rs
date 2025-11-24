/*!
JSON Schema generator and settings.

This module is useful if you want more control over how the schema generated than the [`schema_for!`] macro gives you.
There are two main types in this module:
* [`SchemaSettings`], which defines what JSON Schema features should be used when generating schemas (for example, how `Option`s should be represented).
* [`SchemaGenerator`], which manages the generation of a schema document.
*/

use crate::consts::meta_schemas;
use crate::Schema;
use crate::_alloc_prelude::*;
use crate::{transform::*, JsonSchema};
use alloc::collections::{BTreeMap, BTreeSet};
use core::{any::Any, fmt::Debug};
use dyn_clone::DynClone;
use serde::Serialize;
use serde_json::{Map as JsonMap, Value};

type CowStr = alloc::borrow::Cow<'static, str>;

/// Settings to customize how Schemas are generated.
///
/// The default settings currently conform to [JSON Schema 2020-12](https://json-schema.org/specification-links#2020-12), but this is liable to change in a future version of Schemars if support for other JSON Schema versions is added.
/// If you rely on generated schemas conforming to draft 2020-12, consider using the
/// [`SchemaSettings::draft2020_12()`] method.
#[derive(Debug, Clone)]
#[non_exhaustive]
#[allow(clippy::struct_excessive_bools)]
pub struct SchemaSettings {
    /// A JSON pointer to the expected location of referenceable subschemas within the resulting
    /// root schema.
    ///
    /// A single leading `#` and/or single trailing `/` are ignored.
    ///
    /// Defaults to `"/$defs"`.
    pub definitions_path: CowStr,
    /// The URI of the meta-schema describing the structure of the generated schemas.
    ///
    /// Defaults to [`meta_schemas::DRAFT2020_12`] (`https://json-schema.org/draft/2020-12/schema`).
    pub meta_schema: Option<CowStr>,
    /// A list of [`Transform`]s that get applied to generated root schemas.
    ///
    /// Defaults to an empty vec (no transforms).
    pub transforms: Vec<Box<dyn GenTransform>>,
    /// Inline all subschemas instead of using references.
    ///
    /// Some references may still be generated in schemas for recursive types.
    ///
    /// Defaults to `false`.
    pub inline_subschemas: bool,
    /// Whether the generated schemas should describe how types are serialized or *de*serialized.
    ///
    /// Defaults to `Contract::Deserialize`.
    pub contract: Contract,
    /// Whether to include enum variant names in their schema's `title` when using the [untagged
    /// enum representation](https://serde.rs/enum-representations.html#untagged).
    ///
    /// This setting is respected by `#[derive(JsonSchema)]` on enums, but manual implementations
    /// of `JsonSchema` may ignore this setting.
    ///
    /// Defaults to `false`.
    pub untagged_enum_variant_titles: bool,
}

impl Default for SchemaSettings {
    /// The default settings currently conform to [JSON Schema 2020-12](https://json-schema.org/specification-links#2020-12),
    /// but this is liable to change in a future version of Schemars if support for other JSON Schema versions is added.
    /// If you rely on generated schemas conforming to draft 2020-12, consider using [`SchemaSettings::draft2020_12()`] instead.
    fn default() -> SchemaSettings {
        SchemaSettings::draft2020_12()
    }
}

impl SchemaSettings {
    /// Creates `SchemaSettings` that conform to [JSON Schema Draft 7](https://json-schema.org/specification-links#draft-7).
    pub fn draft07() -> SchemaSettings {
        SchemaSettings {
            definitions_path: "/definitions".into(),
            meta_schema: Some(meta_schemas::DRAFT07.into()),
            transforms: vec![
                Box::new(ReplaceUnevaluatedProperties),
                Box::new(RemoveRefSiblings),
                Box::new(ReplacePrefixItems),
            ],
            inline_subschemas: false,
            contract: Contract::Deserialize,
            untagged_enum_variant_titles: false,
        }
    }

    /// Creates `SchemaSettings` that conform to [JSON Schema 2019-09](https://json-schema.org/specification-links#draft-2019-09-(formerly-known-as-draft-8)).
    pub fn draft2019_09() -> SchemaSettings {
        SchemaSettings {
            definitions_path: "/$defs".into(),
            meta_schema: Some(meta_schemas::DRAFT2019_09.into()),
            transforms: vec![Box::new(ReplacePrefixItems)],
            inline_subschemas: false,
            contract: Contract::Deserialize,
            untagged_enum_variant_titles: false,
        }
    }

    /// Creates `SchemaSettings` that conform to [JSON Schema 2020-12](https://json-schema.org/specification-links#2020-12).
    pub fn draft2020_12() -> SchemaSettings {
        SchemaSettings {
            definitions_path: "/$defs".into(),
            meta_schema: Some(meta_schemas::DRAFT2020_12.into()),
            transforms: Vec::new(),
            inline_subschemas: false,
            contract: Contract::Deserialize,
            untagged_enum_variant_titles: false,
        }
    }

    /// Creates `SchemaSettings` that conform to [OpenAPI 3.0](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#schema).
    pub fn openapi3() -> SchemaSettings {
        SchemaSettings {
            definitions_path: "/components/schemas".into(),
            meta_schema: Some(meta_schemas::OPENAPI3.into()),
            transforms: vec![
                Box::new(ReplaceUnevaluatedProperties),
                Box::new(ReplaceBoolSchemas {
                    skip_additional_properties: true,
                }),
                Box::new(AddNullable::default()),
                Box::new(RemoveRefSiblings),
                Box::new(SetSingleExample),
                Box::new(ReplaceConstValue),
                Box::new(ReplacePrefixItems),
            ],
            inline_subschemas: false,
            contract: Contract::Deserialize,
            untagged_enum_variant_titles: false,
        }
    }

    /// Modifies the `SchemaSettings` by calling the given function.
    ///
    /// # Example
    /// ```
    /// use schemars::generate::{SchemaGenerator, SchemaSettings};
    ///
    /// let settings = SchemaSettings::default().with(|s| {
    ///     s.meta_schema = None;
    ///     s.inline_subschemas = true;
    /// });
    /// let generator = settings.into_generator();
    /// ```
    pub fn with(mut self, configure_fn: impl FnOnce(&mut Self)) -> Self {
        configure_fn(&mut self);
        self
    }

    /// Appends the given transform to the list of [transforms](SchemaSettings::transforms) for
    /// these `SchemaSettings`.
    pub fn with_transform(mut self, transform: impl Transform + Clone + 'static + Send) -> Self {
        self.transforms.push(Box::new(transform));
        self
    }

    /// Creates a new [`SchemaGenerator`] using these settings.
    pub fn into_generator(self) -> SchemaGenerator {
        SchemaGenerator::new(self)
    }

    /// Updates the settings to generate schemas describing how types are **deserialized**.
    pub fn for_deserialize(mut self) -> Self {
        self.contract = Contract::Deserialize;
        self
    }

    /// Updates the settings to generate schemas describing how types are **serialized**.
    pub fn for_serialize(mut self) -> Self {
        self.contract = Contract::Serialize;
        self
    }
}

/// A setting to specify whether generated schemas should describe how types are serialized or
/// *de*serialized.
///
/// This enum is marked as `#[non_exhaustive]` to reserve space to introduce further variants
/// in future.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[allow(missing_docs)]
#[non_exhaustive]
pub enum Contract {
    Deserialize,
    Serialize,
}

impl Contract {
    /// Returns true if `self` is the `Deserialize` contract.
    pub fn is_deserialize(&self) -> bool {
        self == &Contract::Deserialize
    }

    /// Returns true if `self` is the `Serialize` contract.
    pub fn is_serialize(&self) -> bool {
        self == &Contract::Serialize
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SchemaUid(CowStr, Contract);

/// The main type used to generate JSON Schemas.
///
/// # Example
/// ```
/// use schemars::{JsonSchema, SchemaGenerator};
///
/// #[derive(JsonSchema)]
/// struct MyStruct {
///     foo: i32,
/// }
///
/// let generator = SchemaGenerator::default();
/// let schema = generator.into_root_schema_for::<MyStruct>();
/// ```
#[derive(Debug)]
pub struct SchemaGenerator {
    settings: SchemaSettings,
    definitions: JsonMap<String, Value>,
    pending_schema_ids: BTreeSet<SchemaUid>,
    schema_id_to_name: BTreeMap<SchemaUid, CowStr>,
    used_schema_names: BTreeSet<CowStr>,
    // It's unlikely that `root_schema_id_stack` will ever contain more than one item, but it is
    // possible, e.g. if a `json_schema()` implementation calls `generator.root_schema_for<...>()`
    root_schema_id_stack: Vec<SchemaUid>,
}

impl Default for SchemaGenerator {
    fn default() -> Self {
        SchemaSettings::default().into_generator()
    }
}

impl Clone for SchemaGenerator {
    fn clone(&self) -> Self {
        Self {
            settings: self.settings.clone(),
            definitions: self.definitions.clone(),
            pending_schema_ids: BTreeSet::new(),
            schema_id_to_name: BTreeMap::new(),
            used_schema_names: BTreeSet::new(),
            root_schema_id_stack: Vec::new(),
        }
    }
}

impl From<SchemaSettings> for SchemaGenerator {
    fn from(settings: SchemaSettings) -> Self {
        settings.into_generator()
    }
}

impl SchemaGenerator {
    /// Creates a new `SchemaGenerator` using the given settings.
    pub fn new(settings: SchemaSettings) -> SchemaGenerator {
        SchemaGenerator {
            settings,
            definitions: JsonMap::new(),
            pending_schema_ids: BTreeSet::new(),
            schema_id_to_name: BTreeMap::new(),
            used_schema_names: BTreeSet::new(),
            root_schema_id_stack: Vec::new(),
        }
    }

    /// Borrows the [`SchemaSettings`] being used by this `SchemaGenerator`.
    ///
    /// # Example
    /// ```
    /// use schemars::SchemaGenerator;
    ///
    /// let generator = SchemaGenerator::default();
    /// let settings = generator.settings();
    ///
    /// assert_eq!(settings.inline_subschemas, false);
    /// ```
    pub fn settings(&self) -> &SchemaSettings {
        &self.settings
    }

    /// Generates a JSON Schema for the type `T`, and returns either the schema itself or a `$ref`
    /// schema referencing `T`'s schema.
    ///
    /// If `T` is not [inlined](JsonSchema::inline_schema), this will add `T`'s schema to
    /// this generator's definitions, and return a `$ref` schema referencing that schema.
    /// Otherwise, this method behaves identically to [`JsonSchema::json_schema`].
    ///
    /// If `T`'s schema depends on any [non-inlined](JsonSchema::inline_schema) schemas, then
    /// this method will add them to the `SchemaGenerator`'s schema definitions.
    pub fn subschema_for<T: ?Sized + JsonSchema>(&mut self) -> Schema {
        struct FindRef {
            schema: Schema,
            name_to_be_inserted: Option<CowStr>,
        }

        /// Non-generic inner function to improve compile times.
        fn find_ref(
            this: &mut SchemaGenerator,
            uid: &SchemaUid,
            inline_schema: bool,
            schema_name: fn() -> CowStr,
        ) -> Option<FindRef> {
            let return_ref = !inline_schema
                && (!this.settings.inline_subschemas || this.pending_schema_ids.contains(uid));

            if !return_ref {
                return None;
            }

            if this.root_schema_id_stack.last() == Some(uid) {
                return Some(FindRef {
                    schema: Schema::new_ref("#".to_owned()),
                    name_to_be_inserted: None,
                });
            }

            let name = this.schema_id_to_name.get(uid).cloned().unwrap_or_else(|| {
                let base_name = schema_name();
                let mut name = CowStr::Borrowed("");

                if this.used_schema_names.contains(base_name.as_ref()) {
                    for i in 2.. {
                        name = format!("{base_name}{i}").into();
                        if !this.used_schema_names.contains(&name) {
                            break;
                        }
                    }
                } else {
                    name = base_name;
                }

                this.used_schema_names.insert(name.clone());
                this.schema_id_to_name.insert(uid.clone(), name.clone());
                name
            });

            let reference = format!(
                "#{}/{}",
                this.definitions_path_stripped(),
                crate::encoding::encode_ref_name(&name)
            );

            Some(FindRef {
                schema: Schema::new_ref(reference),
                name_to_be_inserted: (!this.definitions().contains_key(name.as_ref()))
                    .then_some(name),
            })
        }

        let uid = self.schema_uid::<T>();

        let Some(FindRef {
            schema,
            name_to_be_inserted,
        }) = find_ref(self, &uid, T::inline_schema(), T::schema_name)
        else {
            return self.json_schema_internal::<T>(&uid);
        };

        if let Some(name) = name_to_be_inserted {
            self.insert_new_subschema_for::<T>(name, &uid);
        }

        schema
    }

    fn insert_new_subschema_for<T: ?Sized + JsonSchema>(&mut self, name: CowStr, uid: &SchemaUid) {
        // TODO: If we've already added a schema for T with the "opposite" contract, then check
        // whether the new schema is identical. If so, re-use the original for both contracts.

        let dummy = false.into();
        // insert into definitions BEFORE calling json_schema to avoid infinite recursion
        self.definitions.insert(name.clone().into(), dummy);

        let schema = self.json_schema_internal::<T>(uid);

        self.definitions.insert(name.into(), schema.to_value());
    }

    /// Borrows the collection of all [non-inlined](JsonSchema::inline_schema) schemas that
    /// have been generated.
    ///
    /// The keys of the returned `Map` are the [schema names](JsonSchema::schema_name), and the
    /// values are the schemas themselves.
    pub fn definitions(&self) -> &JsonMap<String, Value> {
        &self.definitions
    }

    /// Mutably borrows the collection of all [non-inlined](JsonSchema::inline_schema)
    /// schemas that have been generated.
    ///
    /// The keys of the returned `Map` are the [schema names](JsonSchema::schema_name), and the
    /// values are the schemas themselves.
    pub fn definitions_mut(&mut self) -> &mut JsonMap<String, Value> {
        &mut self.definitions
    }

    /// Returns the collection of all [non-inlined](JsonSchema::inline_schema) schemas that
    /// have been generated, leaving an empty `Map` in its place.
    ///
    /// The keys of the returned `Map` are the [schema names](JsonSchema::schema_name), and the
    /// values are the schemas themselves.
    ///
    /// To apply this generator's [transforms](SchemaSettings::transforms) to each of the returned
    /// schemas, set `apply_transforms` to `true`.
    pub fn take_definitions(&mut self, apply_transforms: bool) -> JsonMap<String, Value> {
        let mut definitions = core::mem::take(&mut self.definitions);

        if apply_transforms {
            for schema in definitions.values_mut().flat_map(<&mut Schema>::try_from) {
                self.apply_transforms(schema);
            }
        }

        definitions
    }

    /// Returns an iterator over the [transforms](SchemaSettings::transforms) being used by this
    /// `SchemaGenerator`.
    pub fn transforms_mut(&mut self) -> impl Iterator<Item = &mut dyn GenTransform> {
        self.settings.transforms.iter_mut().map(Box::as_mut)
    }

    /// Generates a JSON Schema for the type `T`.
    ///
    /// If `T`'s schema depends on any [non-inlined](JsonSchema::inline_schema) schemas, then
    /// this method will include them in the returned `Schema` at the [definitions
    /// path](SchemaSettings::definitions_path) (by default `"$defs"`).
    pub fn root_schema_for<T: ?Sized + JsonSchema>(&mut self) -> Schema {
        let schema_uid = self.schema_uid::<T>();
        self.root_schema_id_stack.push(schema_uid.clone());

        let mut schema = self.json_schema_internal::<T>(&schema_uid);

        let object = schema.ensure_object();

        object
            .entry("title")
            .or_insert_with(|| T::schema_name().into());

        if let Some(meta_schema) = self.settings.meta_schema.as_deref() {
            object.insert("$schema".into(), meta_schema.into());
        }

        self.add_definitions(object, self.definitions.clone());
        self.apply_transforms(&mut schema);

        self.root_schema_id_stack.pop();

        schema
    }

    /// Consumes `self` and generates a JSON Schema for the type `T`.
    ///
    /// If `T`'s schema depends on any [non-inlined](JsonSchema::inline_schema) schemas, then
    /// this method will include them in the returned `Schema` at the [definitions
    /// path](SchemaSettings::definitions_path) (by default `"$defs"`).
    pub fn into_root_schema_for<T: ?Sized + JsonSchema>(mut self) -> Schema {
        let schema_uid = self.schema_uid::<T>();
        self.root_schema_id_stack.push(schema_uid.clone());

        let mut schema = self.json_schema_internal::<T>(&schema_uid);

        let object = schema.ensure_object();

        object
            .entry("title")
            .or_insert_with(|| T::schema_name().into());

        if let Some(meta_schema) = core::mem::take(&mut self.settings.meta_schema) {
            object.insert("$schema".into(), meta_schema.into());
        }

        let definitions = self.take_definitions(false);
        self.add_definitions(object, definitions);
        self.apply_transforms(&mut schema);

        schema
    }

    /// Generates a JSON Schema for the given example value.
    ///
    /// If the value implements [`JsonSchema`], then prefer using the
    /// [`root_schema_for()`](Self::root_schema_for()) function which will generally produce a
    /// more precise schema, particularly when the value contains any enums.
    ///
    /// If the `Serialize` implementation of the value decides to fail, this will return an [`Err`].
    pub fn root_schema_for_value<T: ?Sized + Serialize>(
        &mut self,
        value: &T,
    ) -> Result<Schema, serde_json::Error> {
        let mut schema = value.serialize(crate::ser::Serializer {
            generator: self,
            include_title: true,
        })?;

        let object = schema.ensure_object();

        if let Ok(example) = serde_json::to_value(value) {
            object.insert("examples".into(), vec![example].into());
        }

        if let Some(meta_schema) = self.settings.meta_schema.as_deref() {
            object.insert("$schema".into(), meta_schema.into());
        }

        self.add_definitions(object, self.definitions.clone());
        self.apply_transforms(&mut schema);

        Ok(schema)
    }

    /// Consumes `self` and generates a JSON Schema for the given example value.
    ///
    /// If the value  implements [`JsonSchema`], then prefer using the
    /// [`into_root_schema_for()!`](Self::into_root_schema_for()) function which will generally
    /// produce a more precise schema, particularly when the value contains any enums.
    ///
    /// If the `Serialize` implementation of the value decides to fail, this will return an [`Err`].
    pub fn into_root_schema_for_value<T: ?Sized + Serialize>(
        mut self,
        value: &T,
    ) -> Result<Schema, serde_json::Error> {
        let mut schema = value.serialize(crate::ser::Serializer {
            generator: &mut self,
            include_title: true,
        })?;

        let object = schema.ensure_object();

        if let Ok(example) = serde_json::to_value(value) {
            object.insert("examples".into(), vec![example].into());
        }

        if let Some(meta_schema) = core::mem::take(&mut self.settings.meta_schema) {
            object.insert("$schema".into(), meta_schema.into());
        }

        let definitions = self.take_definitions(false);
        self.add_definitions(object, definitions);
        self.apply_transforms(&mut schema);

        Ok(schema)
    }

    /// Returns a reference to the [contract](SchemaSettings::contract) for the settings on this
    /// `SchemaGenerator`.
    ///
    /// This specifies whether generated schemas describe serialize or *de*serialize behaviour.
    pub fn contract(&self) -> &Contract {
        &self.settings.contract
    }

    fn json_schema_internal<T: ?Sized + JsonSchema>(&mut self, uid: &SchemaUid) -> Schema {
        let did_add = self.pending_schema_ids.insert(uid.clone());

        let schema = T::json_schema(self);

        if did_add {
            self.pending_schema_ids.remove(uid);
        }

        schema
    }

    fn add_definitions(
        &mut self,
        schema_object: &mut JsonMap<String, Value>,
        mut definitions: JsonMap<String, Value>,
    ) {
        if definitions.is_empty() {
            return;
        }

        let pointer = self.definitions_path_stripped();
        let Some(target) = json_pointer_mut(schema_object, pointer, true) else {
            return;
        };

        target.append(&mut definitions);
    }

    fn apply_transforms(&mut self, schema: &mut Schema) {
        for transform in self.transforms_mut() {
            transform.transform(schema);
        }
    }

    /// Returns `self.settings.definitions_path` as a plain JSON pointer to the definitions object,
    /// i.e. without a leading '#' or trailing '/'
    fn definitions_path_stripped(&self) -> &str {
        let path = &self.settings.definitions_path;
        let path = path.strip_prefix('#').unwrap_or(path);
        path.strip_suffix('/').unwrap_or(path)
    }

    fn schema_uid<T: ?Sized + JsonSchema>(&self) -> SchemaUid {
        SchemaUid(T::schema_id(), self.settings.contract.clone())
    }
}

fn json_pointer_mut<'a>(
    mut object: &'a mut JsonMap<String, Value>,
    pointer: &str,
    create_if_missing: bool,
) -> Option<&'a mut JsonMap<String, Value>> {
    use serde_json::map::Entry;

    let pointer = pointer.strip_prefix('/')?;
    if pointer.is_empty() {
        return Some(object);
    }

    for mut segment in pointer.split('/') {
        let replaced: String;
        if segment.contains('~') {
            replaced = segment.replace("~1", "/").replace("~0", "~");
            segment = &replaced;
        }

        let next_value = match object.entry(segment) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) if create_if_missing => v.insert(Value::Object(JsonMap::new())),
            Entry::Vacant(_) => return None,
        };

        object = next_value.as_object_mut()?;
    }

    Some(object)
}

/// A [`Transform`] which implements additional traits required to be included in a
/// [`SchemaSettings`].
///
/// You will rarely need to use this trait directly as it is automatically implemented for any type
/// which implements all of:
/// - [`Transform`]
/// - [`std::any::Any`] (implemented for all `'static` types)
/// - [`std::clone::Clone`]
/// - [`std::marker::Send`]
///
/// # Example
/// ```
/// use schemars::transform::Transform;
/// use schemars::generate::GenTransform;
///
/// #[derive(Debug, Clone)]
/// struct MyTransform;
///
/// impl Transform for MyTransform {
///   fn transform(&mut self, schema: &mut schemars::Schema) {
///     todo!()
///   }
/// }
///
/// let v: &dyn GenTransform = &MyTransform;
/// assert!(v.is::<MyTransform>());
/// ```
pub trait GenTransform: Transform + DynClone + Any + Send {
    #[deprecated = "Only to support pre-1.86 rustc"]
    #[doc(hidden)]
    fn _as_any(&self) -> &dyn Any;

    #[deprecated = "Only to support pre-1.86 rustc"]
    #[doc(hidden)]
    fn _as_any_mut(&mut self) -> &mut dyn Any;

    #[deprecated = "Only to support pre-1.86 rustc"]
    #[doc(hidden)]
    fn _into_any(self: Box<Self>) -> Box<dyn Any>;
}

#[allow(deprecated, clippy::used_underscore_items)]
impl dyn GenTransform {
    /// Returns `true` if the inner transform is of type `T`.
    pub fn is<T: Transform + Clone + Any + Send>(&self) -> bool {
        self._as_any().is::<T>()
    }

    /// Returns some reference to the inner transform if it is of type `T`, or
    /// `None` if it isn't.
    ///
    /// # Example
    /// To remove a specific transform from an instance of `SchemaSettings`:
    /// ```
    /// use schemars::generate::SchemaSettings;
    /// use schemars::transform::ReplaceBoolSchemas;
    ///
    /// let mut settings = SchemaSettings::openapi3();
    /// let original_len = settings.transforms.len();
    ///
    /// settings.transforms.retain(|t| !t.is::<ReplaceBoolSchemas>());
    ///
    /// assert_eq!(settings.transforms.len(), original_len - 1);
    /// ```
    pub fn downcast_ref<T: Transform + Clone + Any + Send>(&self) -> Option<&T> {
        self._as_any().downcast_ref::<T>()
    }

    /// Returns some mutable reference to the inner transform if it is of type `T`, or
    /// `None` if it isn't.
    ///
    /// # Example
    /// To modify a specific transform in an instance of `SchemaSettings`:
    /// ```
    /// use schemars::generate::SchemaSettings;
    /// use schemars::transform::ReplaceBoolSchemas;
    ///
    /// let mut settings = SchemaSettings::openapi3();
    /// for t in &mut settings.transforms {
    ///     if let Some(replace_bool_schemas) = t.downcast_mut::<ReplaceBoolSchemas>() {
    ///         replace_bool_schemas.skip_additional_properties = false;
    ///     }
    /// }
    /// ```
    pub fn downcast_mut<T: Transform + Clone + Any + Send>(&mut self) -> Option<&mut T> {
        self._as_any_mut().downcast_mut::<T>()
    }

    /// Attempts to downcast the box to a concrete type.
    ///
    /// If the inner transform is not of type `T`, this returns `self` wrapped in an `Err` so that
    /// it can still be used.
    #[allow(clippy::missing_panics_doc)] // should never panic - `is()` ensures that downcast succeeds
    pub fn downcast<T: Transform + Clone + Any + Send>(
        self: Box<Self>,
    ) -> Result<Box<T>, Box<Self>> {
        if self.is::<T>() {
            Ok(self._into_any().downcast().unwrap())
        } else {
            Err(self)
        }
    }
}

dyn_clone::clone_trait_object!(GenTransform);

impl<T> GenTransform for T
where
    T: Transform + Clone + Any + Send,
{
    fn _as_any(&self) -> &dyn Any {
        self
    }

    fn _as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn _into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

impl Debug for Box<dyn GenTransform> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        #[allow(clippy::used_underscore_items)]
        self._debug_type_name(f)
    }
}

fn _assert_send() {
    fn assert<T: Send>() {}

    assert::<SchemaSettings>();
    assert::<SchemaGenerator>();
}
