//! Serde `Deserializer` module.
//!
//! Due to the complexity of the XML standard and the fact that Serde was developed
//! with JSON in mind, not all Serde concepts apply smoothly to XML. This leads to
//! that fact that some XML concepts are inexpressible in terms of Serde derives
//! and may require manual deserialization.
//!
//! The most notable restriction is the ability to distinguish between _elements_
//! and _attributes_, as no other format used by serde has such a conception.
//!
//! Due to that the mapping is performed in a best effort manner.
//!
//!
//!
//! Table of Contents
//! =================
//! - [Mapping XML to Rust types](#mapping-xml-to-rust-types)
//!   - [Basics](#basics)
//!   - [Optional attributes and elements](#optional-attributes-and-elements)
//!   - [Choices (`xs:choice` XML Schema type)](#choices-xschoice-xml-schema-type)
//!   - [Sequences (`xs:all` and `xs:sequence` XML Schema types)](#sequences-xsall-and-xssequence-xml-schema-types)
//! - [Mapping of `xsi:nil`](#mapping-of-xsinil)
//! - [Generate Rust types from XML](#generate-rust-types-from-xml)
//! - [Composition Rules](#composition-rules)
//! - [Enum Representations](#enum-representations)
//!   - [Normal enum variant](#normal-enum-variant)
//!   - [`$text` enum variant](#text-enum-variant)
//! - [`$text` and `$value` special names](#text-and-value-special-names)
//!   - [`$text`](#text)
//!   - [`$value`](#value)
//!     - [Primitives and sequences of primitives](#primitives-and-sequences-of-primitives)
//!     - [Structs and sequences of structs](#structs-and-sequences-of-structs)
//!     - [Enums and sequences of enums](#enums-and-sequences-of-enums)
//! - [Frequently Used Patterns](#frequently-used-patterns)
//!   - [`<element>` lists](#element-lists)
//!   - [Overlapped (Out-of-Order) Elements](#overlapped-out-of-order-elements)
//!   - [Internally Tagged Enums](#internally-tagged-enums)
//!
//!
//!
//! Mapping XML to Rust types
//! =========================
//!
//! Type names are never considered when deserializing, so you can name your
//! types as you wish. Other general rules:
//! - `struct` field name could be represented in XML only as an attribute name
//!   or an element name;
//! - `enum` variant name could be represented in XML only as an attribute name
//!   or an element name;
//! - the unit struct, unit type `()` and unit enum variant can be deserialized
//!   from any valid XML content:
//!   - attribute and element names;
//!   - attribute and element values;
//!   - text or CDATA content (including mixed text and CDATA content).
//!
//! <div style="background:rgba(120,145,255,0.45);padding:0.75em;">
//!
//! NOTE: All tests are marked with an `ignore` option, even though they do
//! compile. This is  because rustdoc marks such blocks with an information
//! icon unlike `no_run` blocks.
//!
//! </div>
//!
//! <table>
//! <thead>
//! <tr><th colspan="2">
//!
//! ## Basics
//!
//! </th></tr>
//! <tr><th>To parse all these XML's...</th><th>...use these Rust type(s)</th></tr>
//! </thead>
//! <tbody style="vertical-align:top;">
//! <tr>
//! <td>
//! Content of attributes and text / CDATA content of elements (including mixed
//! text and CDATA content):
//!
//! ```xml
//! <... ...="content" />
//! ```
//! ```xml
//! <...>content</...>
//! ```
//! ```xml
//! <...><![CDATA[content]]></...>
//! ```
//! ```xml
//! <...>text<![CDATA[cdata]]>text</...>
//! ```
//! Mixed text / CDATA content represents one logical string, `"textcdatatext"` in that case.
//! </td>
//! <td>
//!
//! You can use any type that can be deserialized from an `&str`, for example:
//! - [`String`] and [`&str`]
//! - [`Cow<str>`]
//! - [`u32`], [`f32`] and other numeric types
//! - `enum`s, like
//!   ```
//!   # use pretty_assertions::assert_eq;
//!   # use serde::Deserialize;
//!   # #[derive(Debug, PartialEq)]
//!   #[derive(Deserialize)]
//!   enum Language {
//!     Rust,
//!     Cpp,
//!     #[serde(other)]
//!     Other,
//!   }
//!   # #[derive(Debug, PartialEq, Deserialize)]
//!   # struct X { #[serde(rename = "$text")] x: Language }
//!   # assert_eq!(X { x: Language::Rust  }, quick_xml::de::from_str("<x>Rust</x>").unwrap());
//!   # assert_eq!(X { x: Language::Cpp   }, quick_xml::de::from_str("<x>C<![CDATA[p]]>p</x>").unwrap());
//!   # assert_eq!(X { x: Language::Other }, quick_xml::de::from_str("<x><![CDATA[other]]></x>").unwrap());
//!   ```
//!
//! <div style="background:rgba(120,145,255,0.45);padding:0.75em;">
//!
//! NOTE: deserialization to non-owned types (i.e. borrow from the input),
//! such as `&str`, is possible only if you parse document in the UTF-8
//! encoding and content does not contain entity references such as `&amp;`,
//! or character references such as `&#xD;`, as well as text content represented
//! by one piece of [text] or [CDATA] element.
//! </div>
//! <!-- TODO: document an error type returned -->
//!
//! [text]: Event::Text
//! [CDATA]: Event::CData
//! </td>
//! </tr>
//! <!-- 2 ===================================================================================== -->
//! <tr>
//! <td>
//!
//! Content of attributes and text / CDATA content of elements (including mixed
//! text and CDATA content), which represents a space-delimited lists, as
//! specified in the XML Schema specification for [`xs:list`] `simpleType`:
//!
//! ```xml
//! <... ...="element1 element2 ..." />
//! ```
//! ```xml
//! <...>
//!   element1
//!   element2
//!   ...
//! </...>
//! ```
//! ```xml
//! <...><![CDATA[
//!   element1
//!   element2
//!   ...
//! ]]></...>
//! ```
//!
//! [`xs:list`]: https://www.w3.org/TR/xmlschema11-2/#list-datatypes
//! </td>
//! <td>
//!
//! Use any type that deserialized using [`deserialize_seq()`] call, for example:
//!
//! ```
//! type List = Vec<u32>;
//! ```
//!
//! See the next row to learn where in your struct definition you should
//! use that type.
//!
//! According to the XML Schema specification, delimiters for elements is one
//! or more space (`' '`, `'\r'`, `'\n'`, and `'\t'`) character(s).
//!
//! <div style="background:rgba(120,145,255,0.45);padding:0.75em;">
//!
//! NOTE: according to the XML Schema restrictions, you cannot escape those
//! white-space characters, so list elements will _never_ contain them.
//! In practice you will usually use `xs:list`s for lists of numbers or enumerated
//! values which looks like identifiers in many languages, for example, `item`,
//! `some_item` or `some-item`, so that shouldn't be a problem.
//!
//! NOTE: according to the XML Schema specification, list elements can be
//! delimited only by spaces. Other delimiters (for example, commas) are not
//! allowed.
//!
//! </div>
//!
//! [`deserialize_seq()`]: de::Deserializer::deserialize_seq
//! </td>
//! </tr>
//! <!-- 3 ===================================================================================== -->
//! <tr>
//! <td>
//! A typical XML with attributes. The root tag name does not matter:
//!
//! ```xml
//! <any-tag one="..." two="..."/>
//! ```
//! </td>
//! <td>
//!
//! A structure where each XML attribute is mapped to a field with a name
//! starting with `@`. Because Rust identifiers do not permit the `@` character,
//! you should use the `#[serde(rename = "@...")]` attribute to rename it.
//! The name of the struct itself does not matter:
//!
//! ```
//! # use serde::Deserialize;
//! # type T = ();
//! # type U = ();
//! // Get both attributes
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! struct AnyName {
//!   #[serde(rename = "@one")]
//!   one: T,
//!
//!   #[serde(rename = "@two")]
//!   two: U,
//! }
//! # quick_xml::de::from_str::<AnyName>(r#"<any-tag one="..." two="..."/>"#).unwrap();
//! ```
//! ```
//! # use serde::Deserialize;
//! # type T = ();
//! // Get only the one attribute, ignore the other
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! struct AnyName {
//!   #[serde(rename = "@one")]
//!   one: T,
//! }
//! # quick_xml::de::from_str::<AnyName>(r#"<any-tag one="..." two="..."/>"#).unwrap();
//! # quick_xml::de::from_str::<AnyName>(r#"<any-tag one="..."/>"#).unwrap();
//! # quick_xml::de::from_str::<AnyName>(r#"<any-tag one="..."><one>...</one></any-tag>"#).unwrap();
//! ```
//! ```
//! # use serde::Deserialize;
//! // Ignore all attributes
//! // You can also use the `()` type (unit type)
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! struct AnyName;
//! # quick_xml::de::from_str::<AnyName>(r#"<any-tag one="..." two="..."/>"#).unwrap();
//! # quick_xml::de::from_str::<AnyName>(r#"<any-tag one="..."><one>...</one></any-tag>"#).unwrap();
//! # quick_xml::de::from_str::<AnyName>(r#"<any-tag><one>...</one><two>...</two></any-tag>"#).unwrap();
//! ```
//!
//! All these structs can be used to deserialize from an XML on the
//! left side depending on amount of information that you want to get.
//! Of course, you can combine them with elements extractor structs (see below).
//!
//! <div style="background:rgba(120,145,255,0.45);padding:0.75em;">
//!
//! NOTE: XML allows you to have an attribute and an element with the same name
//! inside the one element. quick-xml deals with that by prepending a `@` prefix
//! to the name of attributes.
//! </div>
//! </td>
//! </tr>
//! <!-- 4 ===================================================================================== -->
//! <tr>
//! <td>
//! A typical XML with child elements. The root tag name does not matter:
//!
//! ```xml
//! <any-tag>
//!   <one>...</one>
//!   <two>...</two>
//! </any-tag>
//! ```
//! </td>
//! <td>
//! A structure where each XML child element is mapped to the field.
//! Each element name becomes a name of field. The name of the struct itself
//! does not matter:
//!
//! ```
//! # use serde::Deserialize;
//! # type T = ();
//! # type U = ();
//! // Get both elements
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! struct AnyName {
//!   one: T,
//!   two: U,
//! }
//! # quick_xml::de::from_str::<AnyName>(r#"<any-tag><one>...</one><two>...</two></any-tag>"#).unwrap();
//! #
//! # quick_xml::de::from_str::<AnyName>(r#"<any-tag one="..." two="..."/>"#).unwrap_err();
//! # quick_xml::de::from_str::<AnyName>(r#"<any-tag one="..."><two>...</two></any-tag>"#).unwrap_err();
//! ```
//! ```
//! # use serde::Deserialize;
//! # type T = ();
//! // Get only the one element, ignore the other
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! struct AnyName {
//!   one: T,
//! }
//! # quick_xml::de::from_str::<AnyName>(r#"<any-tag><one>...</one><two>...</two></any-tag>"#).unwrap();
//! # quick_xml::de::from_str::<AnyName>(r#"<any-tag one="..."><one>...</one></any-tag>"#).unwrap();
//! ```
//! ```
//! # use serde::Deserialize;
//! // Ignore all elements
//! // You can also use the `()` type (unit type)
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! struct AnyName;
//! # quick_xml::de::from_str::<AnyName>(r#"<any-tag one="..." two="..."/>"#).unwrap();
//! # quick_xml::de::from_str::<AnyName>(r#"<any-tag><one>...</one><two>...</two></any-tag>"#).unwrap();
//! # quick_xml::de::from_str::<AnyName>(r#"<any-tag one="..."><two>...</two></any-tag>"#).unwrap();
//! # quick_xml::de::from_str::<AnyName>(r#"<any-tag one="..."><one>...</one></any-tag>"#).unwrap();
//! ```
//!
//! All these structs can be used to deserialize from an XML on the
//! left side depending on amount of information that you want to get.
//! Of course, you can combine them with attributes extractor structs (see above).
//!
//! <div style="background:rgba(120,145,255,0.45);padding:0.75em;">
//!
//! NOTE: XML allows you to have an attribute and an element with the same name
//! inside the one element. quick-xml deals with that by prepending a `@` prefix
//! to the name of attributes.
//! </div>
//! </td>
//! </tr>
//! <!-- 5 ===================================================================================== -->
//! <tr>
//! <td>
//! An XML with an attribute and a child element named equally:
//!
//! ```xml
//! <any-tag field="...">
//!   <field>...</field>
//! </any-tag>
//! ```
//! </td>
//! <td>
//!
//! You MUST specify `#[serde(rename = "@field")]` on a field that will be used
//! for an attribute:
//!
//! ```
//! # use pretty_assertions::assert_eq;
//! # use serde::Deserialize;
//! # type T = ();
//! # type U = ();
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! struct AnyName {
//!   #[serde(rename = "@field")]
//!   attribute: T,
//!   field: U,
//! }
//! # assert_eq!(
//! #   AnyName { attribute: (), field: () },
//! #   quick_xml::de::from_str(r#"
//! #     <any-tag field="...">
//! #       <field>...</field>
//! #     </any-tag>
//! #   "#).unwrap(),
//! # );
//! ```
//! </td>
//! </tr>
//! <!-- ======================================================================================= -->
//! <tr><th colspan="2">
//!
//! ## Optional attributes and elements
//!
//! </th></tr>
//! <tr><th>To parse all these XML's...</th><th>...use these Rust type(s)</th></tr>
//! <!-- 6 ===================================================================================== -->
//! <tr>
//! <td>
//! An optional XML attribute that you want to capture.
//! The root tag name does not matter:
//!
//! ```xml
//! <any-tag optional="..."/>
//! ```
//! ```xml
//! <any-tag/>
//! ```
//! </td>
//! <td>
//!
//! A structure with an optional field, renamed according to the requirements
//! for attributes:
//!
//! ```
//! # use pretty_assertions::assert_eq;
//! # use serde::Deserialize;
//! # type T = ();
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! struct AnyName {
//!   #[serde(rename = "@optional")]
//!   optional: Option<T>,
//! }
//! # assert_eq!(AnyName { optional: Some(()) }, quick_xml::de::from_str(r#"<any-tag optional="..."/>"#).unwrap());
//! # assert_eq!(AnyName { optional: None     }, quick_xml::de::from_str(r#"<any-tag/>"#).unwrap());
//! ```
//! When the XML attribute is present, type `T` will be deserialized from
//! an attribute value (which is a string). Note, that if `T = String` or other
//! string type, the empty attribute is mapped to a `Some("")`, whereas `None`
//! represents the missed attribute:
//! ```xml
//! <any-tag optional="..."/><!-- Some("...") -->
//! <any-tag optional=""/>   <!-- Some("") -->
//! <any-tag/>               <!-- None -->
//! ```
//! <div style="background:rgba(120,145,255,0.45);padding:0.75em;">
//!
//! NOTE: The behaviour is not symmetric by default. `None` will be serialized as
//! `optional=""`. This behaviour is consistent across serde crates. You should add
//! `#[serde(skip_serializing_if = "Option::is_none")]` attribute to the field to
//! skip `None`s.
//! </div>
//! </td>
//! </tr>
//! <!-- 7 ===================================================================================== -->
//! <tr>
//! <td>
//! An optional XML elements that you want to capture.
//! The root tag name does not matter:
//!
//! ```xml
//! <any-tag/>
//!   <optional>...</optional>
//! </any-tag>
//! ```
//! ```xml
//! <any-tag/>
//!   <optional/>
//! </any-tag>
//! ```
//! ```xml
//! <any-tag/>
//! ```
//! </td>
//! <td>
//!
//! A structure with an optional field:
//!
//! ```
//! # use pretty_assertions::assert_eq;
//! # use serde::Deserialize;
//! # type T = ();
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! struct AnyName {
//!   optional: Option<T>,
//! }
//! # assert_eq!(AnyName { optional: Some(()) }, quick_xml::de::from_str(r#"<any-tag><optional>...</optional></any-tag>"#).unwrap());
//! # assert_eq!(AnyName { optional: None     }, quick_xml::de::from_str(r#"<any-tag/>"#).unwrap());
//! ```
//! When the XML element is present, type `T` will be deserialized from an
//! element (which is a string or a multi-mapping -- i.e. mapping which can have
//! duplicated keys).
//! <div style="background:rgba(120,145,255,0.45);padding:0.75em;">
//!
//! NOTE: The behaviour is not symmetric by default. `None` will be serialized as
//! `<optional/>`. This behaviour is consistent across serde crates. You should add
//! `#[serde(skip_serializing_if = "Option::is_none")]` attribute to the field to
//! skip `None`s.
//!
//! NOTE: Deserializer will automatically handle a [`xsi:nil`] attribute and set field to `None`.
//! For more info see [Mapping of `xsi:nil`](#mapping-of-xsinil).
//! </div>
//! </td>
//! </tr>
//! <!-- ======================================================================================= -->
//! <tr><th colspan="2">
//!
//! ## Choices (`xs:choice` XML Schema type)
//!
//! </th></tr>
//! <tr><th>To parse all these XML's...</th><th>...use these Rust type(s)</th></tr>
//! <!-- 8 ===================================================================================== -->
//! <tr>
//! <td>
//! An XML with different root tag names, as well as text / CDATA content:
//!
//! ```xml
//! <one field1="...">...</one>
//! ```
//! ```xml
//! <two>
//!   <field2>...</field2>
//! </two>
//! ```
//! ```xml
//! Text <![CDATA[or (mixed)
//! CDATA]]> content
//! ```
//! </td>
//! <td>
//!
//! An enum where each variant has the name of a possible root tag. The name of
//! the enum itself does not matter.
//!
//! If you need to get the textual content, mark a variant with `#[serde(rename = "$text")]`.
//!
//! All these structs can be used to deserialize from any XML on the
//! left side depending on amount of information that you want to get:
//!
//! ```
//! # use pretty_assertions::assert_eq;
//! # use serde::Deserialize;
//! # type T = ();
//! # type U = ();
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! #[serde(rename_all = "snake_case")]
//! enum AnyName {
//!   One { #[serde(rename = "@field1")] field1: T },
//!   Two { field2: U },
//!
//!   /// Use unit variant, if you do not care of a content.
//!   /// You can use tuple variant if you want to parse
//!   /// textual content as an xs:list.
//!   /// Struct variants are will pass a string to the
//!   /// struct enum variant visitor, which typically
//!   /// returns Err(Custom)
//!   #[serde(rename = "$text")]
//!   Text(String),
//! }
//! # assert_eq!(AnyName::One { field1: () }, quick_xml::de::from_str(r#"<one field1="...">...</one>"#).unwrap());
//! # assert_eq!(AnyName::Two { field2: () }, quick_xml::de::from_str(r#"<two><field2>...</field2></two>"#).unwrap());
//! # assert_eq!(AnyName::Text("text  cdata ".into()), quick_xml::de::from_str(r#"text <![CDATA[ cdata ]]>"#).unwrap());
//! ```
//! ```
//! # use pretty_assertions::assert_eq;
//! # use serde::Deserialize;
//! # type T = ();
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! struct Two {
//!   field2: T,
//! }
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! #[serde(rename_all = "snake_case")]
//! enum AnyName {
//!   // `field1` content discarded
//!   One,
//!   Two(Two),
//!   #[serde(rename = "$text")]
//!   Text,
//! }
//! # assert_eq!(AnyName::One,                     quick_xml::de::from_str(r#"<one field1="...">...</one>"#).unwrap());
//! # assert_eq!(AnyName::Two(Two { field2: () }), quick_xml::de::from_str(r#"<two><field2>...</field2></two>"#).unwrap());
//! # assert_eq!(AnyName::Text,                    quick_xml::de::from_str(r#"text <![CDATA[ cdata ]]>"#).unwrap());
//! ```
//! ```
//! # use pretty_assertions::assert_eq;
//! # use serde::Deserialize;
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! #[serde(rename_all = "snake_case")]
//! enum AnyName {
//!   One,
//!   // the <two> and textual content will be mapped to this
//!   #[serde(other)]
//!   Other,
//! }
//! # assert_eq!(AnyName::One,   quick_xml::de::from_str(r#"<one field1="...">...</one>"#).unwrap());
//! # assert_eq!(AnyName::Other, quick_xml::de::from_str(r#"<two><field2>...</field2></two>"#).unwrap());
//! # assert_eq!(AnyName::Other, quick_xml::de::from_str(r#"text <![CDATA[ cdata ]]>"#).unwrap());
//! ```
//! <div style="background:rgba(120,145,255,0.45);padding:0.75em;">
//!
//! NOTE: You should have variants for all possible tag names in your enum
//! or have an `#[serde(other)]` variant.
//! <!-- TODO: document an error type if that requirement is violated -->
//! </div>
//! </td>
//! </tr>
//! <!-- 9 ===================================================================================== -->
//! <tr>
//! <td>
//!
//! `<xs:choice>` embedded in the other element, and at the same time you want
//! to get access to other attributes that can appear in the same container
//! (`<any-tag>`). Also this case can be described, as if you want to choose
//! Rust enum variant based on a tag name:
//!
//! ```xml
//! <any-tag field="...">
//!   <one>...</one>
//! </any-tag>
//! ```
//! ```xml
//! <any-tag field="...">
//!   <two>...</two>
//! </any-tag>
//! ```
//! ```xml
//! <any-tag field="...">
//!   Text <![CDATA[or (mixed)
//!   CDATA]]> content
//! </any-tag>
//! ```
//! </td>
//! <td>
//!
//! A structure with a field which type is an `enum`.
//!
//! If you need to get a textual content, mark a variant with `#[serde(rename = "$text")]`.
//!
//! Names of the enum, struct, and struct field with `Choice` type does not matter:
//!
//! ```
//! # use pretty_assertions::assert_eq;
//! # use serde::Deserialize;
//! # type T = ();
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! #[serde(rename_all = "snake_case")]
//! enum Choice {
//!   One,
//!   Two,
//!
//!   /// Use unit variant, if you do not care of a content.
//!   /// You can use tuple variant if you want to parse
//!   /// textual content as an xs:list.
//!   /// Struct variants are will pass a string to the
//!   /// struct enum variant visitor, which typically
//!   /// returns Err(Custom)
//!   #[serde(rename = "$text")]
//!   Text(String),
//! }
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! struct AnyName {
//!   #[serde(rename = "@field")]
//!   field: T,
//!
//!   #[serde(rename = "$value")]
//!   any_name: Choice,
//! }
//! # assert_eq!(
//! #   AnyName { field: (), any_name: Choice::One },
//! #   quick_xml::de::from_str(r#"<any-tag field="..."><one>...</one></any-tag>"#).unwrap(),
//! # );
//! # assert_eq!(
//! #   AnyName { field: (), any_name: Choice::Two },
//! #   quick_xml::de::from_str(r#"<any-tag field="..."><two>...</two></any-tag>"#).unwrap(),
//! # );
//! # assert_eq!(
//! #   AnyName { field: (), any_name: Choice::Text("text  cdata ".into()) },
//! #   quick_xml::de::from_str(r#"<any-tag field="...">text <![CDATA[ cdata ]]></any-tag>"#).unwrap(),
//! # );
//! ```
//! </td>
//! </tr>
//! <!-- 10 ==================================================================================== -->
//! <tr>
//! <td>
//!
//! `<xs:choice>` embedded in the other element, and at the same time you want
//! to get access to other elements that can appear in the same container
//! (`<any-tag>`). Also this case can be described, as if you want to choose
//! Rust enum variant based on a tag name:
//!
//! ```xml
//! <any-tag>
//!   <field>...</field>
//!   <one>...</one>
//! </any-tag>
//! ```
//! ```xml
//! <any-tag>
//!   <two>...</two>
//!   <field>...</field>
//! </any-tag>
//! ```
//! </td>
//! <td>
//!
//! A structure with a field which type is an `enum`.
//!
//! Names of the enum, struct, and struct field with `Choice` type does not matter:
//!
//! ```
//! # use pretty_assertions::assert_eq;
//! # use serde::Deserialize;
//! # type T = ();
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! #[serde(rename_all = "snake_case")]
//! enum Choice {
//!   One,
//!   Two,
//! }
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! struct AnyName {
//!   field: T,
//!
//!   #[serde(rename = "$value")]
//!   any_name: Choice,
//! }
//! # assert_eq!(
//! #   AnyName { field: (), any_name: Choice::One },
//! #   quick_xml::de::from_str(r#"<any-tag><field>...</field><one>...</one></any-tag>"#).unwrap(),
//! # );
//! # assert_eq!(
//! #   AnyName { field: (), any_name: Choice::Two },
//! #   quick_xml::de::from_str(r#"<any-tag><two>...</two><field>...</field></any-tag>"#).unwrap(),
//! # );
//! ```
//!
//! <div style="background:rgba(120,145,255,0.45);padding:0.75em;">
//!
//! NOTE: if your `Choice` enum would contain an `#[serde(other)]`
//! variant, element `<field>` will be mapped to the `field` and not to the enum
//! variant.
//! </div>
//!
//! </td>
//! </tr>
//! <!-- 11 ==================================================================================== -->
//! <tr>
//! <td>
//!
//! `<xs:choice>` encapsulated in other element with a fixed name:
//!
//! ```xml
//! <any-tag field="...">
//!   <choice>
//!     <one>...</one>
//!   </choice>
//! </any-tag>
//! ```
//! ```xml
//! <any-tag field="...">
//!   <choice>
//!     <two>...</two>
//!   </choice>
//! </any-tag>
//! ```
//! </td>
//! <td>
//!
//! A structure with a field of an intermediate type with one field of `enum` type.
//! Actually, this example is not necessary, because you can construct it by yourself
//! using the composition rules that were described above. However the XML construction
//! described here is very common, so it is shown explicitly.
//!
//! Names of the enum and struct does not matter:
//!
//! ```
//! # use pretty_assertions::assert_eq;
//! # use serde::Deserialize;
//! # type T = ();
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! #[serde(rename_all = "snake_case")]
//! enum Choice {
//!   One,
//!   Two,
//! }
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! struct Holder {
//!   #[serde(rename = "$value")]
//!   any_name: Choice,
//! }
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! struct AnyName {
//!   #[serde(rename = "@field")]
//!   field: T,
//!
//!   choice: Holder,
//! }
//! # assert_eq!(
//! #   AnyName { field: (), choice: Holder { any_name: Choice::One } },
//! #   quick_xml::de::from_str(r#"<any-tag field="..."><choice><one>...</one></choice></any-tag>"#).unwrap(),
//! # );
//! # assert_eq!(
//! #   AnyName { field: (), choice: Holder { any_name: Choice::Two } },
//! #   quick_xml::de::from_str(r#"<any-tag field="..."><choice><two>...</two></choice></any-tag>"#).unwrap(),
//! # );
//! ```
//! </td>
//! </tr>
//! <!-- 12 ==================================================================================== -->
//! <tr>
//! <td>
//!
//! `<xs:choice>` encapsulated in other element with a fixed name:
//!
//! ```xml
//! <any-tag>
//!   <field>...</field>
//!   <choice>
//!     <one>...</one>
//!   </choice>
//! </any-tag>
//! ```
//! ```xml
//! <any-tag>
//!   <choice>
//!     <two>...</two>
//!   </choice>
//!   <field>...</field>
//! </any-tag>
//! ```
//! </td>
//! <td>
//!
//! A structure with a field of an intermediate type with one field of `enum` type.
//! Actually, this example is not necessary, because you can construct it by yourself
//! using the composition rules that were described above. However the XML construction
//! described here is very common, so it is shown explicitly.
//!
//! Names of the enum and struct does not matter:
//!
//! ```
//! # use pretty_assertions::assert_eq;
//! # use serde::Deserialize;
//! # type T = ();
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! #[serde(rename_all = "snake_case")]
//! enum Choice {
//!   One,
//!   Two,
//! }
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! struct Holder {
//!   #[serde(rename = "$value")]
//!   any_name: Choice,
//! }
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! struct AnyName {
//!   field: T,
//!
//!   choice: Holder,
//! }
//! # assert_eq!(
//! #   AnyName { field: (), choice: Holder { any_name: Choice::One } },
//! #   quick_xml::de::from_str(r#"<any-tag><field>...</field><choice><one>...</one></choice></any-tag>"#).unwrap(),
//! # );
//! # assert_eq!(
//! #   AnyName { field: (), choice: Holder { any_name: Choice::Two } },
//! #   quick_xml::de::from_str(r#"<any-tag><choice><two>...</two></choice><field>...</field></any-tag>"#).unwrap(),
//! # );
//! ```
//! </td>
//! </tr>
//! <!-- ======================================================================================== -->
//! <tr><th colspan="2">
//!
//! ## Sequences (`xs:all` and `xs:sequence` XML Schema types)
//!
//! </th></tr>
//! <tr><th>To parse all these XML's...</th><th>...use these Rust type(s)</th></tr>
//! <!-- 13 ==================================================================================== -->
//! <tr>
//! <td>
//! A sequence inside of a tag without a dedicated name:
//!
//! ```xml
//! <any-tag/>
//! ```
//! ```xml
//! <any-tag>
//!   <item/>
//! </any-tag>
//! ```
//! ```xml
//! <any-tag>
//!   <item/>
//!   <item/>
//!   <item/>
//! </any-tag>
//! ```
//! </td>
//! <td>
//!
//! A structure with a field which is a sequence type, for example, [`Vec`].
//! Because XML syntax does not distinguish between empty sequences and missed
//! elements, we should indicate that on the Rust side, because serde will require
//! that field `item` exists. You can do that in two possible ways:
//!
//! Use the `#[serde(default)]` attribute for a [field] or the entire [struct]:
//! ```
//! # use pretty_assertions::assert_eq;
//! # use serde::Deserialize;
//! # type Item = ();
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! struct AnyName {
//!   #[serde(default)]
//!   item: Vec<Item>,
//! }
//! # assert_eq!(
//! #   AnyName { item: vec![] },
//! #   quick_xml::de::from_str(r#"<any-tag/>"#).unwrap(),
//! # );
//! # assert_eq!(
//! #   AnyName { item: vec![()] },
//! #   quick_xml::de::from_str(r#"<any-tag><item/></any-tag>"#).unwrap(),
//! # );
//! # assert_eq!(
//! #   AnyName { item: vec![(), (), ()] },
//! #   quick_xml::de::from_str(r#"<any-tag><item/><item/><item/></any-tag>"#).unwrap(),
//! # );
//! ```
//!
//! Use the [`Option`]. In that case inner array will always contains at least one
//! element after deserialization:
//! ```ignore
//! # use pretty_assertions::assert_eq;
//! # use serde::Deserialize;
//! # type Item = ();
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! struct AnyName {
//!   item: Option<Vec<Item>>,
//! }
//! # assert_eq!(
//! #   AnyName { item: None },
//! #   quick_xml::de::from_str(r#"<any-tag/>"#).unwrap(),
//! # );
//! # assert_eq!(
//! #   AnyName { item: Some(vec![()]) },
//! #   quick_xml::de::from_str(r#"<any-tag><item/></any-tag>"#).unwrap(),
//! # );
//! # assert_eq!(
//! #   AnyName { item: Some(vec![(), (), ()]) },
//! #   quick_xml::de::from_str(r#"<any-tag><item/><item/><item/></any-tag>"#).unwrap(),
//! # );
//! ```
//!
//! See also [Frequently Used Patterns](#element-lists).
//!
//! [field]: https://serde.rs/field-attrs.html#default
//! [struct]: https://serde.rs/container-attrs.html#default
//! </td>
//! </tr>
//! <!-- 14 ==================================================================================== -->
//! <tr>
//! <td>
//! A sequence with a strict order, probably with mixed content
//! (text / CDATA and tags):
//!
//! ```xml
//! <one>...</one>
//! text
//! <![CDATA[cdata]]>
//! <two>...</two>
//! <one>...</one>
//! ```
//! <div style="background:rgba(120,145,255,0.45);padding:0.75em;">
//!
//! NOTE: this is just an example for showing mapping. XML does not allow
//! multiple root tags -- you should wrap the sequence into a tag.
//! </div>
//! </td>
//! <td>
//!
//! All elements mapped to the heterogeneous sequential type: tuple or named tuple.
//! Each element of the tuple should be able to be deserialized from the nested
//! element content (`...`), except the enum types which would be deserialized
//! from the full element (`<one>...</one>`), so they could use the element name
//! to choose the right variant:
//!
//! ```
//! # use pretty_assertions::assert_eq;
//! # use serde::Deserialize;
//! # type One = ();
//! # type Two = ();
//! # /*
//! type One = ...;
//! type Two = ...;
//! # */
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! struct AnyName(One, String, Two, One);
//! # assert_eq!(
//! #   AnyName((), "text cdata".into(), (), ()),
//! #   quick_xml::de::from_str(r#"<one>...</one>text <![CDATA[cdata]]><two>...</two><one>...</one>"#).unwrap(),
//! # );
//! ```
//! ```
//! # use pretty_assertions::assert_eq;
//! # use serde::Deserialize;
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! #[serde(rename_all = "snake_case")]
//! enum Choice {
//!   One,
//! }
//! # type Two = ();
//! # /*
//! type Two = ...;
//! # */
//! type AnyName = (Choice, String, Two, Choice);
//! # assert_eq!(
//! #   (Choice::One, "text cdata".to_string(), (), Choice::One),
//! #   quick_xml::de::from_str(r#"<one>...</one>text <![CDATA[cdata]]><two>...</two><one>...</one>"#).unwrap(),
//! # );
//! ```
//! <div style="background:rgba(120,145,255,0.45);padding:0.75em;">
//!
//! NOTE: consequent text and CDATA nodes are merged into the one text node,
//! so you cannot have two adjacent string types in your sequence.
//!
//! NOTE: In the case that the list might contain tags that are overlapped with
//! tags that do not correspond to the list you should add the feature [`overlapped-lists`].
//! </div>
//! </td>
//! </tr>
//! <!-- 15 ==================================================================================== -->
//! <tr>
//! <td>
//! A sequence with a non-strict order, probably with a mixed content
//! (text / CDATA and tags).
//!
//! ```xml
//! <one>...</one>
//! text
//! <![CDATA[cdata]]>
//! <two>...</two>
//! <one>...</one>
//! ```
//! <div style="background:rgba(120,145,255,0.45);padding:0.75em;">
//!
//! NOTE: this is just an example for showing mapping. XML does not allow
//! multiple root tags -- you should wrap the sequence into a tag.
//! </div>
//! </td>
//! <td>
//! A homogeneous sequence of elements with a fixed or dynamic size:
//!
//! ```
//! # use pretty_assertions::assert_eq;
//! # use serde::Deserialize;
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! #[serde(rename_all = "snake_case")]
//! enum Choice {
//!   One,
//!   Two,
//!   #[serde(other)]
//!   Other,
//! }
//! type AnyName = [Choice; 4];
//! # assert_eq!(
//! #   [Choice::One, Choice::Other, Choice::Two, Choice::One],
//! #   quick_xml::de::from_str::<AnyName>(r#"<one>...</one>text <![CDATA[cdata]]><two>...</two><one>...</one>"#).unwrap(),
//! # );
//! ```
//! ```
//! # use pretty_assertions::assert_eq;
//! # use serde::Deserialize;
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! #[serde(rename_all = "snake_case")]
//! enum Choice {
//!   One,
//!   Two,
//!   #[serde(rename = "$text")]
//!   Other(String),
//! }
//! type AnyName = Vec<Choice>;
//! # assert_eq!(
//! #   vec![
//! #     Choice::One,
//! #     Choice::Other("text cdata".into()),
//! #     Choice::Two,
//! #     Choice::One,
//! #   ],
//! #   quick_xml::de::from_str::<AnyName>(r#"<one>...</one>text <![CDATA[cdata]]><two>...</two><one>...</one>"#).unwrap(),
//! # );
//! ```
//! <div style="background:rgba(120,145,255,0.45);padding:0.75em;">
//!
//! NOTE: consequent text and CDATA nodes are merged into the one text node,
//! so you cannot have two adjacent string types in your sequence.
//! </div>
//! </td>
//! </tr>
//! <!-- 16 ==================================================================================== -->
//! <tr>
//! <td>
//! A sequence with a strict order, probably with a mixed content,
//! (text and tags) inside of the other element:
//!
//! ```xml
//! <any-tag attribute="...">
//!   <one>...</one>
//!   text
//!   <![CDATA[cdata]]>
//!   <two>...</two>
//!   <one>...</one>
//! </any-tag>
//! ```
//! </td>
//! <td>
//!
//! A structure where all child elements mapped to the one field which have
//! a heterogeneous sequential type: tuple or named tuple. Each element of the
//! tuple should be able to be deserialized from the full element (`<one>...</one>`).
//!
//! You MUST specify `#[serde(rename = "$value")]` on that field:
//!
//! ```
//! # use pretty_assertions::assert_eq;
//! # use serde::Deserialize;
//! # type One = ();
//! # type Two = ();
//! # /*
//! type One = ...;
//! type Two = ...;
//! # */
//!
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! struct AnyName {
//!   #[serde(rename = "@attribute")]
//! # attribute: (),
//! # /*
//!   attribute: ...,
//! # */
//!   // Does not (yet?) supported by the serde
//!   // https://github.com/serde-rs/serde/issues/1905
//!   // #[serde(flatten)]
//!   #[serde(rename = "$value")]
//!   any_name: (One, String, Two, One),
//! }
//! # assert_eq!(
//! #   AnyName { attribute: (), any_name: ((), "text cdata".into(), (), ()) },
//! #   quick_xml::de::from_str("\
//! #     <any-tag attribute='...'>\
//! #       <one>...</one>\
//! #       text \
//! #       <![CDATA[cdata]]>\
//! #       <two>...</two>\
//! #       <one>...</one>\
//! #     </any-tag>"
//! #   ).unwrap(),
//! # );
//! ```
//! ```
//! # use pretty_assertions::assert_eq;
//! # use serde::Deserialize;
//! # type One = ();
//! # type Two = ();
//! # /*
//! type One = ...;
//! type Two = ...;
//! # */
//!
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! struct NamedTuple(One, String, Two, One);
//!
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! struct AnyName {
//!   #[serde(rename = "@attribute")]
//! # attribute: (),
//! # /*
//!   attribute: ...,
//! # */
//!   // Does not (yet?) supported by the serde
//!   // https://github.com/serde-rs/serde/issues/1905
//!   // #[serde(flatten)]
//!   #[serde(rename = "$value")]
//!   any_name: NamedTuple,
//! }
//! # assert_eq!(
//! #   AnyName { attribute: (), any_name: NamedTuple((), "text cdata".into(), (), ()) },
//! #   quick_xml::de::from_str("\
//! #     <any-tag attribute='...'>\
//! #       <one>...</one>\
//! #       text \
//! #       <![CDATA[cdata]]>\
//! #       <two>...</two>\
//! #       <one>...</one>\
//! #     </any-tag>"
//! #   ).unwrap(),
//! # );
//! ```
//! <div style="background:rgba(120,145,255,0.45);padding:0.75em;">
//!
//! NOTE: consequent text and CDATA nodes are merged into the one text node,
//! so you cannot have two adjacent string types in your sequence.
//! </div>
//! </td>
//! </tr>
//! <!-- 17 ==================================================================================== -->
//! <tr>
//! <td>
//! A sequence with a non-strict order, probably with a mixed content
//! (text / CDATA and tags) inside of the other element:
//!
//! ```xml
//! <any-tag>
//!   <one>...</one>
//!   text
//!   <![CDATA[cdata]]>
//!   <two>...</two>
//!   <one>...</one>
//! </any-tag>
//! ```
//! </td>
//! <td>
//!
//! A structure where all child elements mapped to the one field which have
//! a homogeneous sequential type: array-like container. A container type `T`
//! should be able to be deserialized from the nested element content (`...`),
//! except if it is an enum type which would be deserialized from the full
//! element (`<one>...</one>`).
//!
//! You MUST specify `#[serde(rename = "$value")]` on that field:
//!
//! ```
//! # use pretty_assertions::assert_eq;
//! # use serde::Deserialize;
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! #[serde(rename_all = "snake_case")]
//! enum Choice {
//!   One,
//!   Two,
//!   #[serde(rename = "$text")]
//!   Other(String),
//! }
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! struct AnyName {
//!   #[serde(rename = "@attribute")]
//! # attribute: (),
//! # /*
//!   attribute: ...,
//! # */
//!   // Does not (yet?) supported by the serde
//!   // https://github.com/serde-rs/serde/issues/1905
//!   // #[serde(flatten)]
//!   #[serde(rename = "$value")]
//!   any_name: [Choice; 4],
//! }
//! # assert_eq!(
//! #   AnyName { attribute: (), any_name: [
//! #     Choice::One,
//! #     Choice::Other("text cdata".into()),
//! #     Choice::Two,
//! #     Choice::One,
//! #   ] },
//! #   quick_xml::de::from_str("\
//! #     <any-tag attribute='...'>\
//! #       <one>...</one>\
//! #       text \
//! #       <![CDATA[cdata]]>\
//! #       <two>...</two>\
//! #       <one>...</one>\
//! #     </any-tag>"
//! #   ).unwrap(),
//! # );
//! ```
//! ```
//! # use pretty_assertions::assert_eq;
//! # use serde::Deserialize;
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! #[serde(rename_all = "snake_case")]
//! enum Choice {
//!   One,
//!   Two,
//!   #[serde(rename = "$text")]
//!   Other(String),
//! }
//! # #[derive(Debug, PartialEq)]
//! #[derive(Deserialize)]
//! struct AnyName {
//!   #[serde(rename = "@attribute")]
//! # attribute: (),
//! # /*
//!   attribute: ...,
//! # */
//!   // Does not (yet?) supported by the serde
//!   // https://github.com/serde-rs/serde/issues/1905
//!   // #[serde(flatten)]
//!   #[serde(rename = "$value")]
//!   any_name: Vec<Choice>,
//! }
//! # assert_eq!(
//! #   AnyName { attribute: (), any_name: vec![
//! #     Choice::One,
//! #     Choice::Other("text cdata".into()),
//! #     Choice::Two,
//! #     Choice::One,
//! #   ] },
//! #   quick_xml::de::from_str("\
//! #     <any-tag attribute='...'>\
//! #       <one>...</one>\
//! #       text \
//! #       <![CDATA[cdata]]>\
//! #       <two>...</two>\
//! #       <one>...</one>\
//! #     </any-tag>"
//! #   ).unwrap(),
//! # );
//! ```
//! <div style="background:rgba(120,145,255,0.45);padding:0.75em;">
//!
//! NOTE: consequent text and CDATA nodes are merged into the one text node,
//! so you cannot have two adjacent string types in your sequence.
//! </div>
//! </td>
//! </tr>
//! </tbody>
//! </table>
//!
//!
//! Mapping of `xsi:nil`
//! ====================
//!
//! quick-xml supports handling of [`xsi:nil`] special attribute. When field of optional
//! type is mapped to the XML element which have `xsi:nil="true"` set, or if that attribute
//! is placed on parent XML element, the deserializer will call [`Visitor::visit_none`]
//! and skip XML element corresponding to a field.
//!
//! Examples:
//!
//! ```
//! # use pretty_assertions::assert_eq;
//! # use serde::Deserialize;
//! #[derive(Deserialize, Debug, PartialEq)]
//! struct TypeWithOptionalField {
//!   element: Option<String>,
//! }
//!
//! assert_eq!(
//!   TypeWithOptionalField {
//!     element: None,
//!   },
//!   quick_xml::de::from_str("
//!     <any-tag xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'>
//!       <element xsi:nil='true'>Content is skiped because of xsi:nil='true'</element>
//!     </any-tag>
//!   ").unwrap(),
//! );
//! ```
//!
//! You can capture attributes from the optional type, because ` xsi:nil="true"` elements can have
//! attributes:
//! ```
//! # use pretty_assertions::assert_eq;
//! # use serde::Deserialize;
//! #[derive(Deserialize, Debug, PartialEq)]
//! struct TypeWithOptionalField {
//!   #[serde(rename = "@attribute")]
//!   attribute: usize,
//!
//!   element: Option<String>,
//!   non_optional: String,
//! }
//!
//! assert_eq!(
//!   TypeWithOptionalField {
//!     attribute: 42,
//!     element: None,
//!     non_optional: "Note, that non-optional fields will be deserialized as usual".to_string(),
//!   },
//!   quick_xml::de::from_str("
//!     <any-tag attribute='42' xsi:nil='true' xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'>
//!       <element>Content is skiped because of xsi:nil='true'</element>
//!       <non_optional>Note, that non-optional fields will be deserialized as usual</non_optional>
//!     </any-tag>
//!   ").unwrap(),
//! );
//! ```
//!
//! Generate Rust types from XML
//! ============================
//!
//! To speed up the creation of Rust types that represent a given XML file you can
//! use the [xml_schema_generator](https://github.com/Thomblin/xml_schema_generator).
//! It provides a standalone binary and a Rust library that parses one or more XML files
//! and generates a collection of structs that are compatible with quick_xml::de.
//!
//!
//!
//! Composition Rules
//! =================
//!
//! The XML format is very different from other formats supported by `serde`.
//! One such difference it is how data in the serialized form is related to
//! the Rust type. Usually each byte in the data can be associated only with
//! one field in the data structure. However, XML is an exception.
//!
//! For example, took this XML:
//!
//! ```xml
//! <any>
//!   <key attr="value"/>
//! </any>
//! ```
//!
//! and try to deserialize it to the struct `AnyName`:
//!
//! ```no_run
//! # use serde::Deserialize;
//! #[derive(Deserialize)]
//! struct AnyName { // AnyName calls `deserialize_struct` on `<any><key attr="value"/></any>`
//!                  //                         Used data:          ^^^^^^^^^^^^^^^^^^^
//!   key: Inner,    // Inner   calls `deserialize_struct` on `<key attr="value"/>`
//!                  //                         Used data:          ^^^^^^^^^^^^
//! }
//! #[derive(Deserialize)]
//! struct Inner {
//!   #[serde(rename = "@attr")]
//!   attr: String,  // String  calls `deserialize_string` on `value`
//!                  //                         Used data:     ^^^^^
//! }
//! ```
//!
//! Comments shows what methods of a [`Deserializer`] called by each struct
//! `deserialize` method and which input their seen. **Used data** shows, what
//! content is actually used for deserializing. As you see, name of the inner
//! `<key>` tag used both as a map key / outer struct field name and as part
//! of the inner struct (although _value_ of the tag, i.e. `key` is not used
//! by it).
//!
//!
//!
//! Enum Representations
//! ====================
//!
//! `quick-xml` represents enums differently in normal fields, `$text` fields and
//! `$value` fields. A normal representation is compatible with serde's adjacent
//! and internal tags feature -- tag for adjacently and internally tagged enums
//! are serialized using [`Serializer::serialize_unit_variant`] and deserialized
//! using [`Deserializer::deserialize_enum`].
//!
//! Use those simple rules to remember, how enum would be represented in XML:
//! - In `$value` field the representation is always the same as top-level representation;
//! - In `$text` field the representation is always the same as in normal field,
//!   but surrounding tags with field name are removed;
//! - In normal field the representation is always contains a tag with a field name.
//!
//! Normal enum variant
//! -------------------
//!
//! To model an `xs:choice` XML construct use `$value` field.
//! To model a top-level `xs:choice` just use the enum type.
//!
//! |Kind   |Top-level and in `$value` field          |In normal field      |In `$text` field     |
//! |-------|-----------------------------------------|---------------------|---------------------|
//! |Unit   |`<Unit/>`                                |`<field>Unit</field>`|`Unit`               |
//! |Newtype|`<Newtype>42</Newtype>`                  |Err(Custom) [^0]     |Err(Custom) [^0]     |
//! |Tuple  |`<Tuple>42</Tuple><Tuple>answer</Tuple>` |Err(Custom) [^0]     |Err(Custom) [^0]     |
//! |Struct |`<Struct><q>42</q><a>answer</a></Struct>`|Err(Custom) [^0]     |Err(Custom) [^0]     |
//!
//! `$text` enum variant
//! --------------------
//!
//! |Kind   |Top-level and in `$value` field          |In normal field      |In `$text` field     |
//! |-------|-----------------------------------------|---------------------|---------------------|
//! |Unit   |_(empty)_                                |`<field/>`           |_(empty)_            |
//! |Newtype|`42`                                     |Err(Custom) [^0] [^1]|Err(Custom) [^0] [^2]|
//! |Tuple  |`42 answer`                              |Err(Custom) [^0] [^3]|Err(Custom) [^0] [^4]|
//! |Struct |Err(Custom) [^0]                         |Err(Custom) [^0]     |Err(Custom) [^0]     |
//!
//! [^0]: Error is returned by the deserialized type. In case of derived implementation a `Custom`
//!       error will be returned, but custom deserialize implementation can successfully deserialize
//!       value from a string which will be passed to it.
//!
//! [^1]: If this serialize as `<field>42</field>` then it will be ambiguity during deserialization,
//!       because it clash with `Unit` representation in normal field.
//!
//! [^2]: If this serialize as `42` then it will be ambiguity during deserialization,
//!       because it clash with `Unit` representation in `$text` field.
//!
//! [^3]: If this serialize as `<field>42 answer</field>` then it will be ambiguity during deserialization,
//!       because it clash with `Unit` representation in normal field.
//!
//! [^4]: If this serialize as `42 answer` then it will be ambiguity during deserialization,
//!       because it clash with `Unit` representation in `$text` field.
//!
//!
//!
//! `$text` and `$value` special names
//! ==================================
//!
//! quick-xml supports two special names for fields -- `$text` and `$value`.
//! Although they may seem the same, there is a distinction. Two different
//! names is required mostly for serialization, because quick-xml should know
//! how you want to serialize certain constructs, which could be represented
//! through XML in multiple different ways.
//!
//! The only difference is in how complex types and sequences are serialized.
//! If you doubt which one you should select, begin with [`$value`](#value).
//!
//! ## `$text`
//! `$text` is used when you want to write your XML as a text or a CDATA content.
//! More formally, field with that name represents simple type definition with
//! `{variety} = atomic` or `{variety} = union` whose basic members are all atomic,
//! as described in the [specification].
//!
//! As a result, not all types of such fields can be serialized. Only serialization
//! of following types are supported:
//! - all primitive types (strings, numbers, booleans)
//! - unit variants of enumerations (serializes to a name of a variant)
//! - newtypes (delegates serialization to inner type)
//! - [`Option`] of above (`None` serializes to nothing)
//! - sequences (including tuples and tuple variants of enumerations) of above,
//!   excluding `None` and empty string elements (because it will not be possible
//!   to deserialize them back). The elements are separated by space(s)
//! - unit type `()` and unit structs (serializes to nothing)
//!
//! Complex types, such as structs and maps, are not supported in this field.
//! If you want them, you should use `$value`.
//!
//! Sequences serialized to a space-delimited string, that is why only certain
//! types are allowed in this mode:
//!
//! ```
//! # use serde::{Deserialize, Serialize};
//! # use quick_xml::de::from_str;
//! # use quick_xml::se::to_string;
//! #[derive(Deserialize, Serialize, PartialEq, Debug)]
//! struct AnyName {
//!     #[serde(rename = "$text")]
//!     field: Vec<usize>,
//! }
//!
//! let obj = AnyName { field: vec![1, 2, 3] };
//! let xml = to_string(&obj).unwrap();
//! assert_eq!(xml, "<AnyName>1 2 3</AnyName>");
//!
//! let object: AnyName = from_str(&xml).unwrap();
//! assert_eq!(object, obj);
//! ```
//!
//! ## `$value`
//! <div style="background:rgba(120,145,255,0.45);padding:0.75em;">
//!
//! NOTE: a name `#content` would better explain the purpose of that field,
//! but `$value` is used for compatibility with other XML serde crates, which
//! uses that name. This will allow you to switch XML crates more smoothly if required.
//! </div>
//!
//! Representation of primitive types in `$value` does not differ from their
//! representation in `$text` field. The difference is how sequences are serialized.
//! `$value` serializes each sequence item as a separate XML element. The name
//! of that element is taken from serialized type, and because only `enum`s provide
//! such name (their variant name), only they should be used for such fields.
//!
//! `$value` fields does not support `struct` types with fields, the serialization
//! of such types would end with an `Err(Unsupported)`. Unit structs and unit
//! type `()` serializing to nothing and can be deserialized from any content.
//!
//! Serialization and deserialization of `$value` field performed as usual, except
//! that name for an XML element will be given by the serialized type, instead of
//! field. The latter allow to serialize enumerated types, where variant is encoded
//! as a tag name, and, so, represent an XSD `xs:choice` schema by the Rust `enum`.
//!
//! In the example below, field will be serialized as `<field/>`, because elements
//! get their names from the field name. It cannot be deserialized, because `Enum`
//! expects elements `<A/>`, `<B/>` or `<C/>`, but `AnyName` looked only for `<field/>`:
//!
//! ```
//! # use serde::{Deserialize, Serialize};
//! # use pretty_assertions::assert_eq;
//! # #[derive(PartialEq, Debug)]
//! #[derive(Deserialize, Serialize)]
//! enum Enum { A, B, C }
//!
//! # #[derive(PartialEq, Debug)]
//! #[derive(Deserialize, Serialize)]
//! struct AnyName {
//!     // <field>A</field>, <field>B</field>, or <field>C</field>
//!     field: Enum,
//! }
//! # assert_eq!(
//! #     quick_xml::se::to_string(&AnyName { field: Enum::A }).unwrap(),
//! #     "<AnyName><field>A</field></AnyName>",
//! # );
//! # assert_eq!(
//! #     AnyName { field: Enum::B },
//! #     quick_xml::de::from_str("<root><field>B</field></root>").unwrap(),
//! # );
//! ```
//!
//! If you rename field to `$value`, then `field` would be serialized as `<A/>`,
//! `<B/>` or `<C/>`, depending on the its content. It is also possible to
//! deserialize it from the same elements:
//!
//! ```
//! # use serde::{Deserialize, Serialize};
//! # use pretty_assertions::assert_eq;
//! # #[derive(Deserialize, Serialize, PartialEq, Debug)]
//! # enum Enum { A, B, C }
//! #
//! # #[derive(PartialEq, Debug)]
//! #[derive(Deserialize, Serialize)]
//! struct AnyName {
//!     // <A/>, <B/> or <C/>
//!     #[serde(rename = "$value")]
//!     field: Enum,
//! }
//! # assert_eq!(
//! #     quick_xml::se::to_string(&AnyName { field: Enum::A }).unwrap(),
//! #     "<AnyName><A/></AnyName>",
//! # );
//! # assert_eq!(
//! #     AnyName { field: Enum::B },
//! #     quick_xml::de::from_str("<root><B/></root>").unwrap(),
//! # );
//! ```
//!
//! ### Primitives and sequences of primitives
//!
//! Sequences serialized to a list of elements. Note, that types that does not
//! produce their own tag (i. e. primitives) will produce [`SeError::Unsupported`]
//! if they contains more that one element, because such sequence cannot be
//! deserialized to the same value:
//!
//! ```
//! # use serde::{Deserialize, Serialize};
//! # use pretty_assertions::assert_eq;
//! # use quick_xml::de::from_str;
//! # use quick_xml::se::to_string;
//! #[derive(Deserialize, Serialize, PartialEq, Debug)]
//! struct AnyName {
//!     #[serde(rename = "$value")]
//!     field: Vec<usize>,
//! }
//!
//! let obj = AnyName { field: vec![1, 2, 3] };
//! // If this object were serialized, it would be represented as "<AnyName>123</AnyName>"
//! to_string(&obj).unwrap_err();
//!
//! let object: AnyName = from_str("<AnyName>123</AnyName>").unwrap();
//! assert_eq!(object, AnyName { field: vec![123] });
//!
//! // `1 2 3` is mapped to a single `usize` element
//! // It is impossible to deserialize list of primitives to such field
//! from_str::<AnyName>("<AnyName>1 2 3</AnyName>").unwrap_err();
//! ```
//!
//! A particular case of that example is a string `$value` field, which probably
//! would be a most used example of that attribute:
//!
//! ```
//! # use serde::{Deserialize, Serialize};
//! # use pretty_assertions::assert_eq;
//! # use quick_xml::de::from_str;
//! # use quick_xml::se::to_string;
//! #[derive(Deserialize, Serialize, PartialEq, Debug)]
//! struct AnyName {
//!     #[serde(rename = "$value")]
//!     field: String,
//! }
//!
//! let obj = AnyName { field: "content".to_string() };
//! let xml = to_string(&obj).unwrap();
//! assert_eq!(xml, "<AnyName>content</AnyName>");
//! ```
//!
//! ### Structs and sequences of structs
//!
//! Note, that structures do not have a serializable name as well (name of the
//! type is never used), so it is impossible to serialize non-unit struct or
//! sequence of non-unit structs in `$value` field. (sequences of) unit structs
//! are serialized as empty string, because units itself serializing
//! to nothing:
//!
//! ```
//! # use serde::{Deserialize, Serialize};
//! # use pretty_assertions::assert_eq;
//! # use quick_xml::de::from_str;
//! # use quick_xml::se::to_string;
//! #[derive(Deserialize, Serialize, PartialEq, Debug)]
//! struct Unit;
//!
//! #[derive(Deserialize, Serialize, PartialEq, Debug)]
//! struct AnyName {
//!     // #[serde(default)] is required to deserialization of empty lists
//!     // This is a general note, not related to $value
//!     #[serde(rename = "$value", default)]
//!     field: Vec<Unit>,
//! }
//!
//! let obj = AnyName { field: vec![Unit, Unit, Unit] };
//! let xml = to_string(&obj).unwrap();
//! assert_eq!(xml, "<AnyName/>");
//!
//! let object: AnyName = from_str("<AnyName/>").unwrap();
//! assert_eq!(object, AnyName { field: vec![] });
//!
//! let object: AnyName = from_str("<AnyName></AnyName>").unwrap();
//! assert_eq!(object, AnyName { field: vec![] });
//!
//! let object: AnyName = from_str("<AnyName><A/><B/><C/></AnyName>").unwrap();
//! assert_eq!(object, AnyName { field: vec![Unit, Unit, Unit] });
//! ```
//!
//! ### Enums and sequences of enums
//!
//! Enumerations uses the variant name as an element name:
//!
//! ```
//! # use serde::{Deserialize, Serialize};
//! # use pretty_assertions::assert_eq;
//! # use quick_xml::de::from_str;
//! # use quick_xml::se::to_string;
//! #[derive(Deserialize, Serialize, PartialEq, Debug)]
//! struct AnyName {
//!     #[serde(rename = "$value")]
//!     field: Vec<Enum>,
//! }
//!
//! #[derive(Deserialize, Serialize, PartialEq, Debug)]
//! enum Enum { A, B, C }
//!
//! let obj = AnyName { field: vec![Enum::A, Enum::B, Enum::C] };
//! let xml = to_string(&obj).unwrap();
//! assert_eq!(
//!     xml,
//!     "<AnyName>\
//!         <A/>\
//!         <B/>\
//!         <C/>\
//!      </AnyName>"
//! );
//!
//! let object: AnyName = from_str(&xml).unwrap();
//! assert_eq!(object, obj);
//! ```
//!
//! ----------------------------------------------------------------------------
//!
//! You can have either `$text` or `$value` field in your structs. Unfortunately,
//! that is not enforced, so you can theoretically have both, but you should
//! avoid that.
//!
//!
//!
//! Frequently Used Patterns
//! ========================
//!
//! Some XML constructs used so frequent, that it is worth to document the recommended
//! way to represent them in the Rust. The sections below describes them.
//!
//! `<element>` lists
//! -----------------
//! Many XML formats wrap lists of elements in the additional container,
//! although this is not required by the XML rules:
//!
//! ```xml
//! <root>
//!   <field1/>
//!   <field2/>
//!   <list><!-- Container -->
//!     <element/>
//!     <element/>
//!     <element/>
//!   </list>
//!   <field3/>
//! </root>
//! ```
//! In this case, there is a great desire to describe this XML in this way:
//! ```
//! /// Represents <element/>
//! type Element = ();
//!
//! /// Represents <root>...</root>
//! struct AnyName {
//!     // Incorrect
//!     list: Vec<Element>,
//! }
//! ```
//! This will not work, because potentially `<list>` element can have attributes
//! and other elements inside. You should define the struct for the `<list>`
//! explicitly, as you do that in the XSD for that XML:
//! ```
//! /// Represents <element/>
//! type Element = ();
//!
//! /// Represents <root>...</root>
//! struct AnyName {
//!     // Correct
//!     list: List,
//! }
//! /// Represents <list>...</list>
//! struct List {
//!     element: Vec<Element>,
//! }
//! ```
//!
//! If you want to simplify your API, you could write a simple function for unwrapping
//! inner list and apply it via [`deserialize_with`]:
//!
//! ```
//! # use pretty_assertions::assert_eq;
//! use quick_xml::de::from_str;
//! use serde::{Deserialize, Deserializer};
//!
//! /// Represents <element/>
//! type Element = ();
//!
//! /// Represents <root>...</root>
//! #[derive(Deserialize, Debug, PartialEq)]
//! struct AnyName {
//!     #[serde(deserialize_with = "unwrap_list")]
//!     list: Vec<Element>,
//! }
//!
//! fn unwrap_list<'de, D>(deserializer: D) -> Result<Vec<Element>, D::Error>
//! where
//!     D: Deserializer<'de>,
//! {
//!     /// Represents <list>...</list>
//!     #[derive(Deserialize)]
//!     struct List {
//!         // default allows empty list
//!         #[serde(default)]
//!         element: Vec<Element>,
//!     }
//!     Ok(List::deserialize(deserializer)?.element)
//! }
//!
//! assert_eq!(
//!     AnyName { list: vec![(), (), ()] },
//!     from_str("
//!         <root>
//!           <list>
//!             <element/>
//!             <element/>
//!             <element/>
//!           </list>
//!         </root>
//!     ").unwrap(),
//! );
//! ```
//!
//! Instead of writing such functions manually, you also could try <https://lib.rs/crates/serde-query>.
//!
//! Overlapped (Out-of-Order) Elements
//! ----------------------------------
//! In the case that the list might contain tags that are overlapped with
//! tags that do not correspond to the list (this is a usual case in XML
//! documents) like this:
//! ```xml
//! <any-name>
//!   <item/>
//!   <another-item/>
//!   <item/>
//!   <item/>
//! </any-name>
//! ```
//! you should enable the [`overlapped-lists`] feature to make it possible
//! to deserialize this to:
//! ```no_run
//! # use serde::Deserialize;
//! #[derive(Deserialize)]
//! #[serde(rename_all = "kebab-case")]
//! struct AnyName {
//!     item: Vec<()>,
//!     another_item: (),
//! }
//! ```
//!
//!
//! Internally Tagged Enums
//! -----------------------
//! [Tagged enums] are currently not supported because of an issue in the Serde
//! design (see [serde#1183] and [quick-xml#586]) and missing optimizations in
//! Serde which could be useful for XML parsing ([serde#1495]). This can be worked
//! around by manually implementing deserialize with `#[serde(deserialize_with = "func")]`
//! or implementing [`Deserialize`], but this can get very tedious very fast for
//! files with large amounts of tagged enums. To help with this issue quick-xml
//! provides a macro [`impl_deserialize_for_internally_tagged_enum!`]. See the
//! macro documentation for details.
//!
//!
//! [`overlapped-lists`]: ../index.html#overlapped-lists
//! [specification]: https://www.w3.org/TR/xmlschema11-1/#Simple_Type_Definition
//! [`deserialize_with`]: https://serde.rs/field-attrs.html#deserialize_with
//! [`xsi:nil`]: https://www.w3.org/TR/xmlschema-1/#xsi_nil
//! [`Serializer::serialize_unit_variant`]: serde::Serializer::serialize_unit_variant
//! [`Deserializer::deserialize_enum`]: serde::Deserializer::deserialize_enum
//! [`SeError::Unsupported`]: crate::errors::serialize::SeError::Unsupported
//! [Tagged enums]: https://serde.rs/enum-representations.html#internally-tagged
//! [serde#1183]: https://github.com/serde-rs/serde/issues/1183
//! [serde#1495]: https://github.com/serde-rs/serde/issues/1495
//! [quick-xml#586]: https://github.com/tafia/quick-xml/issues/586
//! [`impl_deserialize_for_internally_tagged_enum!`]: crate::impl_deserialize_for_internally_tagged_enum

// Macros should be defined before the modules that using them
// Also, macros should be imported before using them
use serde::serde_if_integer128;

macro_rules! deserialize_num {
    ($deserialize:ident => $visit:ident, $($mut:tt)?) => {
        fn $deserialize<V>($($mut)? self, visitor: V) -> Result<V::Value, DeError>
        where
            V: Visitor<'de>,
        {
            // No need to unescape because valid integer representations cannot be escaped
            let text = self.read_string()?;
            match text.parse() {
                Ok(number) => visitor.$visit(number),
                Err(_) => match text {
                    Cow::Borrowed(t) => visitor.visit_str(t),
                    Cow::Owned(t) => visitor.visit_string(t),
                }
            }
        }
    };
}

/// Implement deserialization methods for scalar types, such as numbers, strings,
/// byte arrays, booleans and identifiers.
macro_rules! deserialize_primitives {
    ($($mut:tt)?) => {
        deserialize_num!(deserialize_i8 => visit_i8, $($mut)?);
        deserialize_num!(deserialize_i16 => visit_i16, $($mut)?);
        deserialize_num!(deserialize_i32 => visit_i32, $($mut)?);
        deserialize_num!(deserialize_i64 => visit_i64, $($mut)?);

        deserialize_num!(deserialize_u8 => visit_u8, $($mut)?);
        deserialize_num!(deserialize_u16 => visit_u16, $($mut)?);
        deserialize_num!(deserialize_u32 => visit_u32, $($mut)?);
        deserialize_num!(deserialize_u64 => visit_u64, $($mut)?);

        serde_if_integer128! {
            deserialize_num!(deserialize_i128 => visit_i128, $($mut)?);
            deserialize_num!(deserialize_u128 => visit_u128, $($mut)?);
        }

        deserialize_num!(deserialize_f32 => visit_f32, $($mut)?);
        deserialize_num!(deserialize_f64 => visit_f64, $($mut)?);

        fn deserialize_bool<V>($($mut)? self, visitor: V) -> Result<V::Value, DeError>
        where
            V: Visitor<'de>,
        {
            let text = match self.read_string()? {
                Cow::Borrowed(s) => CowRef::Input(s),
                Cow::Owned(s) => CowRef::Owned(s),
            };
            text.deserialize_bool(visitor)
        }

        /// Character represented as [strings](#method.deserialize_str).
        #[inline]
        fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, DeError>
        where
            V: Visitor<'de>,
        {
            self.deserialize_str(visitor)
        }

        fn deserialize_str<V>($($mut)? self, visitor: V) -> Result<V::Value, DeError>
        where
            V: Visitor<'de>,
        {
            let text = self.read_string()?;
            match text {
                Cow::Borrowed(string) => visitor.visit_borrowed_str(string),
                Cow::Owned(string) => visitor.visit_string(string),
            }
        }

        /// Representation of owned strings the same as [non-owned](#method.deserialize_str).
        #[inline]
        fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, DeError>
        where
            V: Visitor<'de>,
        {
            self.deserialize_str(visitor)
        }

        /// Forwards deserialization to the [`deserialize_any`](#method.deserialize_any).
        #[inline]
        fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, DeError>
        where
            V: Visitor<'de>,
        {
            self.deserialize_any(visitor)
        }

        /// Forwards deserialization to the [`deserialize_bytes`](#method.deserialize_bytes).
        #[inline]
        fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, DeError>
        where
            V: Visitor<'de>,
        {
            self.deserialize_bytes(visitor)
        }

        /// Representation of the named units the same as [unnamed units](#method.deserialize_unit).
        #[inline]
        fn deserialize_unit_struct<V>(
            self,
            _name: &'static str,
            visitor: V,
        ) -> Result<V::Value, DeError>
        where
            V: Visitor<'de>,
        {
            self.deserialize_unit(visitor)
        }

        /// Representation of tuples the same as [sequences](#method.deserialize_seq).
        #[inline]
        fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, DeError>
        where
            V: Visitor<'de>,
        {
            self.deserialize_seq(visitor)
        }

        /// Representation of named tuples the same as [unnamed tuples](#method.deserialize_tuple).
        #[inline]
        fn deserialize_tuple_struct<V>(
            self,
            _name: &'static str,
            len: usize,
            visitor: V,
        ) -> Result<V::Value, DeError>
        where
            V: Visitor<'de>,
        {
            self.deserialize_tuple(len, visitor)
        }

        /// Forwards deserialization to the [`deserialize_struct`](#method.deserialize_struct)
        /// with empty name and fields.
        #[inline]
        fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, DeError>
        where
            V: Visitor<'de>,
        {
            self.deserialize_struct("", &[], visitor)
        }

        /// Identifiers represented as [strings](#method.deserialize_str).
        #[inline]
        fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, DeError>
        where
            V: Visitor<'de>,
        {
            self.deserialize_str(visitor)
        }

        /// Forwards deserialization to the [`deserialize_unit`](#method.deserialize_unit).
        #[inline]
        fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, DeError>
        where
            V: Visitor<'de>,
        {
            self.deserialize_unit(visitor)
        }
    };
}

mod key;
mod map;
mod resolver;
mod simple_type;
mod text;
mod var;

pub use self::resolver::{EntityResolver, PredefinedEntityResolver};
pub use self::simple_type::SimpleTypeDeserializer;
pub use crate::errors::serialize::DeError;

use crate::{
    de::map::ElementMapAccess,
    encoding::Decoder,
    errors::Error,
    events::{BytesCData, BytesEnd, BytesStart, BytesText, Event},
    name::QName,
    reader::NsReader,
    utils::CowRef,
};
use serde::de::{
    self, Deserialize, DeserializeOwned, DeserializeSeed, IntoDeserializer, SeqAccess, Visitor,
};
use std::borrow::Cow;
#[cfg(feature = "overlapped-lists")]
use std::collections::VecDeque;
use std::io::BufRead;
use std::mem::replace;
#[cfg(feature = "overlapped-lists")]
use std::num::NonZeroUsize;
use std::ops::Deref;

/// Data represented by a text node or a CDATA node. XML markup is not expected
pub(crate) const TEXT_KEY: &str = "$text";
/// Data represented by any XML markup inside
pub(crate) const VALUE_KEY: &str = "$value";

/// Decoded and concatenated content of consequent [`Text`] and [`CData`]
/// events. _Consequent_ means that events should follow each other or be
/// delimited only by (any count of) [`Comment`] or [`PI`] events.
///
/// Internally text is stored in `Cow<str>`. Cloning of text is cheap while it
/// is borrowed and makes copies of data when it is owned.
///
/// [`Text`]: Event::Text
/// [`CData`]: Event::CData
/// [`Comment`]: Event::Comment
/// [`PI`]: Event::PI
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Text<'a> {
    text: Cow<'a, str>,
}

impl<'a> Deref for Text<'a> {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.text.deref()
    }
}

impl<'a> From<&'a str> for Text<'a> {
    #[inline]
    fn from(text: &'a str) -> Self {
        Self {
            text: Cow::Borrowed(text),
        }
    }
}

impl<'a> From<String> for Text<'a> {
    #[inline]
    fn from(text: String) -> Self {
        Self {
            text: Cow::Owned(text),
        }
    }
}

impl<'a> From<Cow<'a, str>> for Text<'a> {
    #[inline]
    fn from(text: Cow<'a, str>) -> Self {
        Self { text }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// Simplified event which contains only these variants that used by deserializer
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeEvent<'a> {
    /// Start tag (with attributes) `<tag attr="value">`.
    Start(BytesStart<'a>),
    /// End tag `</tag>`.
    End(BytesEnd<'a>),
    /// Decoded and concatenated content of consequent [`Text`] and [`CData`]
    /// events. _Consequent_ means that events should follow each other or be
    /// delimited only by (any count of) [`Comment`] or [`PI`] events.
    ///
    /// [`Text`]: Event::Text
    /// [`CData`]: Event::CData
    /// [`Comment`]: Event::Comment
    /// [`PI`]: Event::PI
    Text(Text<'a>),
    /// End of XML document.
    Eof,
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// Simplified event which contains only these variants that used by deserializer,
/// but [`Text`] events not yet fully processed.
///
/// [`Text`] events should be trimmed if they does not surrounded by the other
/// [`Text`] or [`CData`] events. This event contains intermediate state of [`Text`]
/// event, where they are trimmed from the start, but not from the end. To trim
/// end spaces we should lookahead by one deserializer event (i. e. skip all
/// comments and processing instructions).
///
/// [`Text`]: Event::Text
/// [`CData`]: Event::CData
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PayloadEvent<'a> {
    /// Start tag (with attributes) `<tag attr="value">`.
    Start(BytesStart<'a>),
    /// End tag `</tag>`.
    End(BytesEnd<'a>),
    /// Escaped character data between tags.
    Text(BytesText<'a>),
    /// Unescaped character data stored in `<![CDATA[...]]>`.
    CData(BytesCData<'a>),
    /// Document type definition data (DTD) stored in `<!DOCTYPE ...>`.
    DocType(BytesText<'a>),
    /// End of XML document.
    Eof,
}

impl<'a> PayloadEvent<'a> {
    /// Ensures that all data is owned to extend the object's lifetime if necessary.
    #[inline]
    fn into_owned(self) -> PayloadEvent<'static> {
        match self {
            PayloadEvent::Start(e) => PayloadEvent::Start(e.into_owned()),
            PayloadEvent::End(e) => PayloadEvent::End(e.into_owned()),
            PayloadEvent::Text(e) => PayloadEvent::Text(e.into_owned()),
            PayloadEvent::CData(e) => PayloadEvent::CData(e.into_owned()),
            PayloadEvent::DocType(e) => PayloadEvent::DocType(e.into_owned()),
            PayloadEvent::Eof => PayloadEvent::Eof,
        }
    }
}

/// An intermediate reader that consumes [`PayloadEvent`]s and produces final [`DeEvent`]s.
/// [`PayloadEvent::Text`] events, that followed by any event except
/// [`PayloadEvent::Text`] or [`PayloadEvent::CData`], are trimmed from the end.
struct XmlReader<'i, R: XmlRead<'i>, E: EntityResolver = PredefinedEntityResolver> {
    /// A source of low-level XML events
    reader: R,
    /// Intermediate event, that could be returned by the next call to `next()`.
    /// If that is the `Text` event then leading spaces already trimmed, but
    /// trailing spaces is not. Before the event will be returned, trimming of
    /// the spaces could be necessary
    lookahead: Result<PayloadEvent<'i>, DeError>,

    /// Used to resolve unknown entities that would otherwise cause the parser
    /// to return an [`EscapeError::UnrecognizedEntity`] error.
    ///
    /// [`EscapeError::UnrecognizedEntity`]: crate::escape::EscapeError::UnrecognizedEntity
    entity_resolver: E,
}

impl<'i, R: XmlRead<'i>, E: EntityResolver> XmlReader<'i, R, E> {
    fn new(mut reader: R, entity_resolver: E) -> Self {
        // Lookahead by one event immediately, so we do not need to check in the
        // loop if we need lookahead or not
        let lookahead = reader.next();

        Self {
            reader,
            lookahead,
            entity_resolver,
        }
    }

    /// Returns `true` if all events was consumed
    const fn is_empty(&self) -> bool {
        matches!(self.lookahead, Ok(PayloadEvent::Eof))
    }

    /// Read next event and put it in lookahead, return the current lookahead
    #[inline(always)]
    fn next_impl(&mut self) -> Result<PayloadEvent<'i>, DeError> {
        replace(&mut self.lookahead, self.reader.next())
    }

    /// Returns `true` when next event is not a text event in any form.
    #[inline(always)]
    const fn current_event_is_last_text(&self) -> bool {
        // If next event is a text or CDATA, we should not trim trailing spaces
        !matches!(
            self.lookahead,
            Ok(PayloadEvent::Text(_)) | Ok(PayloadEvent::CData(_))
        )
    }

    /// Read all consequent [`Text`] and [`CData`] events until non-text event
    /// occurs. Content of all events would be appended to `result` and returned
    /// as [`DeEvent::Text`].
    ///
    /// [`Text`]: PayloadEvent::Text
    /// [`CData`]: PayloadEvent::CData
    fn drain_text(&mut self, mut result: Cow<'i, str>) -> Result<DeEvent<'i>, DeError> {
        loop {
            if self.current_event_is_last_text() {
                break;
            }

            match self.next_impl()? {
                PayloadEvent::Text(mut e) => {
                    if self.current_event_is_last_text() {
                        // FIXME: Actually, we should trim after decoding text, but now we trim before
                        e.inplace_trim_end();
                    }
                    result
                        .to_mut()
                        .push_str(&e.unescape_with(|entity| self.entity_resolver.resolve(entity))?);
                }
                PayloadEvent::CData(e) => result.to_mut().push_str(&e.decode()?),

                // SAFETY: current_event_is_last_text checks that event is Text or CData
                _ => unreachable!("Only `Text` and `CData` events can come here"),
            }
        }
        Ok(DeEvent::Text(Text { text: result }))
    }

    /// Return an input-borrowing event.
    fn next(&mut self) -> Result<DeEvent<'i>, DeError> {
        loop {
            return match self.next_impl()? {
                PayloadEvent::Start(e) => Ok(DeEvent::Start(e)),
                PayloadEvent::End(e) => Ok(DeEvent::End(e)),
                PayloadEvent::Text(mut e) => {
                    if self.current_event_is_last_text() && e.inplace_trim_end() {
                        // FIXME: Actually, we should trim after decoding text, but now we trim before
                        continue;
                    }
                    self.drain_text(e.unescape_with(|entity| self.entity_resolver.resolve(entity))?)
                }
                PayloadEvent::CData(e) => self.drain_text(e.decode()?),
                PayloadEvent::DocType(e) => {
                    self.entity_resolver
                        .capture(e)
                        .map_err(|err| DeError::Custom(format!("cannot parse DTD: {}", err)))?;
                    continue;
                }
                PayloadEvent::Eof => Ok(DeEvent::Eof),
            };
        }
    }

    #[inline]
    fn read_to_end(&mut self, name: QName) -> Result<(), DeError> {
        match self.lookahead {
            // We pre-read event with the same name that is required to be skipped.
            // First call of `read_to_end` will end out pre-read event, the second
            // will consume other events
            Ok(PayloadEvent::Start(ref e)) if e.name() == name => {
                let result1 = self.reader.read_to_end(name);
                let result2 = self.reader.read_to_end(name);

                // In case of error `next_impl` returns `Eof`
                let _ = self.next_impl();
                result1?;
                result2?;
            }
            // We pre-read event with the same name that is required to be skipped.
            // Because this is end event, we already consume the whole tree, so
            // nothing to do, just update lookahead
            Ok(PayloadEvent::End(ref e)) if e.name() == name => {
                let _ = self.next_impl();
            }
            Ok(_) => {
                let result = self.reader.read_to_end(name);

                // In case of error `next_impl` returns `Eof`
                let _ = self.next_impl();
                result?;
            }
            // Read next lookahead event, unpack error from the current lookahead
            Err(_) => {
                self.next_impl()?;
            }
        }
        Ok(())
    }

    #[inline]
    fn decoder(&self) -> Decoder {
        self.reader.decoder()
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// Deserialize an instance of type `T` from a string of XML text.
pub fn from_str<'de, T>(s: &'de str) -> Result<T, DeError>
where
    T: Deserialize<'de>,
{
    let mut de = Deserializer::from_str(s);
    T::deserialize(&mut de)
}

/// Deserialize from a reader. This method will do internal copies of data
/// read from `reader`. If you want have a `&str` input and want to borrow
/// as much as possible, use [`from_str`].
pub fn from_reader<R, T>(reader: R) -> Result<T, DeError>
where
    R: BufRead,
    T: DeserializeOwned,
{
    let mut de = Deserializer::from_reader(reader);
    T::deserialize(&mut de)
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// A structure that deserializes XML into Rust values.
pub struct Deserializer<'de, R, E: EntityResolver = PredefinedEntityResolver>
where
    R: XmlRead<'de>,
{
    /// An XML reader that streams events into this deserializer
    reader: XmlReader<'de, R, E>,

    /// When deserializing sequences sometimes we have to skip unwanted events.
    /// That events should be stored and then replayed. This is a replay buffer,
    /// that streams events while not empty. When it exhausted, events will
    /// requested from [`Self::reader`].
    #[cfg(feature = "overlapped-lists")]
    read: VecDeque<DeEvent<'de>>,
    /// When deserializing sequences sometimes we have to skip events, because XML
    /// is tolerant to elements order and even if in the XSD order is strictly
    /// specified (using `xs:sequence`) most of XML parsers allows order violations.
    /// That means, that elements, forming a sequence, could be overlapped with
    /// other elements, do not related to that sequence.
    ///
    /// In order to support this, deserializer will scan events and skip unwanted
    /// events, store them here. After call [`Self::start_replay()`] all events
    /// moved from this to [`Self::read`].
    #[cfg(feature = "overlapped-lists")]
    write: VecDeque<DeEvent<'de>>,
    /// Maximum number of events that can be skipped when processing sequences
    /// that occur out-of-order. This field is used to prevent potential
    /// denial-of-service (DoS) attacks which could cause infinite memory
    /// consumption when parsing a very large amount of XML into a sequence field.
    #[cfg(feature = "overlapped-lists")]
    limit: Option<NonZeroUsize>,

    #[cfg(not(feature = "overlapped-lists"))]
    peek: Option<DeEvent<'de>>,

    /// Buffer to store attribute name as a field name exposed to serde consumers
    key_buf: String,
}

impl<'de, R, E> Deserializer<'de, R, E>
where
    R: XmlRead<'de>,
    E: EntityResolver,
{
    /// Create an XML deserializer from one of the possible quick_xml input sources.
    ///
    /// Typically it is more convenient to use one of these methods instead:
    ///
    ///  - [`Deserializer::from_str`]
    ///  - [`Deserializer::from_reader`]
    fn new(reader: R, entity_resolver: E) -> Self {
        Self {
            reader: XmlReader::new(reader, entity_resolver),

            #[cfg(feature = "overlapped-lists")]
            read: VecDeque::new(),
            #[cfg(feature = "overlapped-lists")]
            write: VecDeque::new(),
            #[cfg(feature = "overlapped-lists")]
            limit: None,

            #[cfg(not(feature = "overlapped-lists"))]
            peek: None,

            key_buf: String::new(),
        }
    }

    /// Returns `true` if all events was consumed.
    pub fn is_empty(&self) -> bool {
        #[cfg(feature = "overlapped-lists")]
        if self.read.is_empty() {
            return self.reader.is_empty();
        }
        #[cfg(not(feature = "overlapped-lists"))]
        if self.peek.is_none() {
            return self.reader.is_empty();
        }
        false
    }

    /// Returns the underlying XML reader.
    ///
    /// ```
    /// # use pretty_assertions::assert_eq;
    /// use serde::Deserialize;
    /// use quick_xml::de::Deserializer;
    /// use quick_xml::NsReader;
    ///
    /// #[derive(Deserialize)]
    /// struct SomeStruct {
    ///     field1: String,
    ///     field2: String,
    /// }
    ///
    /// // Try to deserialize from broken XML
    /// let mut de = Deserializer::from_str(
    ///     "<SomeStruct><field1><field2></SomeStruct>"
    /// //   0                           ^= 28        ^= 41
    /// );
    ///
    /// let err = SomeStruct::deserialize(&mut de);
    /// assert!(err.is_err());
    ///
    /// let reader: &NsReader<_> = de.get_ref().get_ref();
    ///
    /// assert_eq!(reader.error_position(), 28);
    /// assert_eq!(reader.buffer_position(), 41);
    /// ```
    pub const fn get_ref(&self) -> &R {
        &self.reader.reader
    }

    /// Set the maximum number of events that could be skipped during deserialization
    /// of sequences.
    ///
    /// If `<element>` contains more than specified nested elements, `$text` or
    /// CDATA nodes, then [`DeError::TooManyEvents`] will be returned during
    /// deserialization of sequence field (any type that uses [`deserialize_seq`]
    /// for the deserialization, for example, `Vec<T>`).
    ///
    /// This method can be used to prevent a [DoS] attack and infinite memory
    /// consumption when parsing a very large XML to a sequence field.
    ///
    /// It is strongly recommended to set limit to some value when you parse data
    /// from untrusted sources. You should choose a value that your typical XMLs
    /// can have _between_ different elements that corresponds to the same sequence.
    ///
    /// # Examples
    ///
    /// Let's imagine, that we deserialize such structure:
    /// ```
    /// struct List {
    ///   item: Vec<()>,
    /// }
    /// ```
    ///
    /// The XML that we try to parse look like this:
    /// ```xml
    /// <any-name>
    ///   <item/>
    ///   <!-- Bufferization starts at this point -->
    ///   <another-item>
    ///     <some-element>with text</some-element>
    ///     <yet-another-element/>
    ///   </another-item>
    ///   <!-- Buffer will be emptied at this point; 7 events were buffered -->
    ///   <item/>
    ///   <!-- There is nothing to buffer, because elements follows each other -->
    ///   <item/>
    /// </any-name>
    /// ```
    ///
    /// There, when we deserialize the `item` field, we need to buffer 7 events,
    /// before we can deserialize the second `<item/>`:
    ///
    /// - `<another-item>`
    /// - `<some-element>`
    /// - `$text(with text)`
    /// - `</some-element>`
    /// - `<yet-another-element/>` (virtual start event)
    /// - `<yet-another-element/>` (virtual end event)
    /// - `</another-item>`
    ///
    /// Note, that `<yet-another-element/>` internally represented as 2 events:
    /// one for the start tag and one for the end tag. In the future this can be
    /// eliminated, but for now we use [auto-expanding feature] of a reader,
    /// because this simplifies deserializer code.
    ///
    /// [`deserialize_seq`]: serde::Deserializer::deserialize_seq
    /// [DoS]: https://en.wikipedia.org/wiki/Denial-of-service_attack
    /// [auto-expanding feature]: crate::reader::Config::expand_empty_elements
    #[cfg(feature = "overlapped-lists")]
    pub fn event_buffer_size(&mut self, limit: Option<NonZeroUsize>) -> &mut Self {
        self.limit = limit;
        self
    }

    #[cfg(feature = "overlapped-lists")]
    fn peek(&mut self) -> Result<&DeEvent<'de>, DeError> {
        if self.read.is_empty() {
            self.read.push_front(self.reader.next()?);
        }
        if let Some(event) = self.read.front() {
            return Ok(event);
        }
        // SAFETY: `self.read` was filled in the code above.
        // NOTE: Can be replaced with `unsafe { std::hint::unreachable_unchecked() }`
        // if unsafe code will be allowed
        unreachable!()
    }
    #[cfg(not(feature = "overlapped-lists"))]
    fn peek(&mut self) -> Result<&DeEvent<'de>, DeError> {
        if self.peek.is_none() {
            self.peek = Some(self.reader.next()?);
        }
        match self.peek.as_ref() {
            Some(v) => Ok(v),
            // SAFETY: a `None` variant for `self.peek` would have been replaced
            // by a `Some` variant in the code above.
            // TODO: Can be replaced with `unsafe { std::hint::unreachable_unchecked() }`
            // if unsafe code will be allowed
            None => unreachable!(),
        }
    }

    #[inline]
    fn last_peeked(&self) -> &DeEvent<'de> {
        #[cfg(feature = "overlapped-lists")]
        {
            self.read
                .front()
                .expect("`Deserializer::peek()` should be called")
        }
        #[cfg(not(feature = "overlapped-lists"))]
        {
            self.peek
                .as_ref()
                .expect("`Deserializer::peek()` should be called")
        }
    }

    fn next(&mut self) -> Result<DeEvent<'de>, DeError> {
        // Replay skipped or peeked events
        #[cfg(feature = "overlapped-lists")]
        if let Some(event) = self.read.pop_front() {
            return Ok(event);
        }
        #[cfg(not(feature = "overlapped-lists"))]
        if let Some(e) = self.peek.take() {
            return Ok(e);
        }
        self.reader.next()
    }

    /// Returns the mark after which all events, skipped by [`Self::skip()`] call,
    /// should be replayed after calling [`Self::start_replay()`].
    #[cfg(feature = "overlapped-lists")]
    #[inline]
    #[must_use = "returned checkpoint should be used in `start_replay`"]
    fn skip_checkpoint(&self) -> usize {
        self.write.len()
    }

    /// Extracts XML tree of events from and stores them in the skipped events
    /// buffer from which they can be retrieved later. You MUST call
    /// [`Self::start_replay()`] after calling this to give access to the skipped
    /// events and release internal buffers.
    #[cfg(feature = "overlapped-lists")]
    fn skip(&mut self) -> Result<(), DeError> {
        let event = self.next()?;
        self.skip_event(event)?;
        match self.write.back() {
            // Skip all subtree, if we skip a start event
            Some(DeEvent::Start(e)) => {
                let end = e.name().as_ref().to_owned();
                let mut depth = 0;
                loop {
                    let event = self.next()?;
                    match event {
                        DeEvent::Start(ref e) if e.name().as_ref() == end => {
                            self.skip_event(event)?;
                            depth += 1;
                        }
                        DeEvent::End(ref e) if e.name().as_ref() == end => {
                            self.skip_event(event)?;
                            if depth == 0 {
                                break;
                            }
                            depth -= 1;
                        }
                        DeEvent::Eof => {
                            self.skip_event(event)?;
                            break;
                        }
                        _ => self.skip_event(event)?,
                    }
                }
            }
            _ => (),
        }
        Ok(())
    }

    #[cfg(feature = "overlapped-lists")]
    #[inline]
    fn skip_event(&mut self, event: DeEvent<'de>) -> Result<(), DeError> {
        if let Some(max) = self.limit {
            if self.write.len() >= max.get() {
                return Err(DeError::TooManyEvents(max));
            }
        }
        self.write.push_back(event);
        Ok(())
    }

    /// Moves buffered events, skipped after given `checkpoint` from [`Self::write`]
    /// skip buffer to [`Self::read`] buffer.
    ///
    /// After calling this method, [`Self::peek()`] and [`Self::next()`] starts
    /// return events that was skipped previously by calling [`Self::skip()`],
    /// and only when all that events will be consumed, the deserializer starts
    /// to drain events from underlying reader.
    ///
    /// This method MUST be called if any number of [`Self::skip()`] was called
    /// after [`Self::new()`] or `start_replay()` or you'll lost events.
    #[cfg(feature = "overlapped-lists")]
    fn start_replay(&mut self, checkpoint: usize) {
        if checkpoint == 0 {
            self.write.append(&mut self.read);
            std::mem::swap(&mut self.read, &mut self.write);
        } else {
            let mut read = self.write.split_off(checkpoint);
            read.append(&mut self.read);
            self.read = read;
        }
    }

    #[inline]
    fn read_string(&mut self) -> Result<Cow<'de, str>, DeError> {
        self.read_string_impl(true)
    }

    /// Consumes consequent [`Text`] and [`CData`] (both a referred below as a _text_)
    /// events, merge them into one string. If there are no such events, returns
    /// an empty string.
    ///
    /// If `allow_start` is `false`, then only text events are consumed, for other
    /// events an error is returned (see table below).
    ///
    /// If `allow_start` is `true`, then two or three events are expected:
    /// - [`DeEvent::Start`];
    /// - _(optional)_ [`DeEvent::Text`] which content is returned;
    /// - [`DeEvent::End`]. If text event was missed, an empty string is returned.
    ///
    /// Corresponding events are consumed.
    ///
    /// # Handling events
    ///
    /// The table below shows how events is handled by this method:
    ///
    /// |Event             |XML                        |Handling
    /// |------------------|---------------------------|----------------------------------------
    /// |[`DeEvent::Start`]|`<tag>...</tag>`           |if `allow_start == true`, result determined by the second table, otherwise emits [`UnexpectedStart("tag")`](DeError::UnexpectedStart)
    /// |[`DeEvent::End`]  |`</any-tag>`               |This is impossible situation, the method will panic if it happens
    /// |[`DeEvent::Text`] |`text content` or `<![CDATA[cdata content]]>` (probably mixed)|Returns event content unchanged
    /// |[`DeEvent::Eof`]  |                           |Emits [`UnexpectedEof`](DeError::UnexpectedEof)
    ///
    /// Second event, consumed if [`DeEvent::Start`] was received and `allow_start == true`:
    ///
    /// |Event             |XML                        |Handling
    /// |------------------|---------------------------|----------------------------------------------------------------------------------
    /// |[`DeEvent::Start`]|`<any-tag>...</any-tag>`   |Emits [`UnexpectedStart("any-tag")`](DeError::UnexpectedStart)
    /// |[`DeEvent::End`]  |`</tag>`                   |Returns an empty slice. The reader guarantee that tag will match the open one
    /// |[`DeEvent::Text`] |`text content` or `<![CDATA[cdata content]]>` (probably mixed)|Returns event content unchanged, expects the `</tag>` after that
    /// |[`DeEvent::Eof`]  |                           |Emits [`InvalidXml(IllFormed(MissingEndTag))`](DeError::InvalidXml)
    ///
    /// [`Text`]: Event::Text
    /// [`CData`]: Event::CData
    fn read_string_impl(&mut self, allow_start: bool) -> Result<Cow<'de, str>, DeError> {
        match self.next()? {
            DeEvent::Text(e) => Ok(e.text),
            // allow one nested level
            DeEvent::Start(e) if allow_start => self.read_text(e.name()),
            DeEvent::Start(e) => Err(DeError::UnexpectedStart(e.name().as_ref().to_owned())),
            // SAFETY: The reader is guaranteed that we don't have unmatched tags
            // If we here, then out deserializer has a bug
            DeEvent::End(e) => unreachable!("{:?}", e),
            DeEvent::Eof => Err(DeError::UnexpectedEof),
        }
    }
    /// Consumes one [`DeEvent::Text`] event and ensures that it is followed by the
    /// [`DeEvent::End`] event.
    ///
    /// # Parameters
    /// - `name`: name of a tag opened before reading text. The corresponding end tag
    ///   should present in input just after the text
    fn read_text(&mut self, name: QName) -> Result<Cow<'de, str>, DeError> {
        match self.next()? {
            DeEvent::Text(e) => match self.next()? {
                // The matching tag name is guaranteed by the reader
                DeEvent::End(_) => Ok(e.text),
                // SAFETY: Cannot be two consequent Text events, they would be merged into one
                DeEvent::Text(_) => unreachable!(),
                DeEvent::Start(e) => Err(DeError::UnexpectedStart(e.name().as_ref().to_owned())),
                DeEvent::Eof => Err(Error::missed_end(name, self.reader.decoder()).into()),
            },
            // We can get End event in case of `<tag></tag>` or `<tag/>` input
            // Return empty text in that case
            // The matching tag name is guaranteed by the reader
            DeEvent::End(_) => Ok("".into()),
            DeEvent::Start(s) => Err(DeError::UnexpectedStart(s.name().as_ref().to_owned())),
            DeEvent::Eof => Err(Error::missed_end(name, self.reader.decoder()).into()),
        }
    }

    /// Drops all events until event with [name](BytesEnd::name()) `name` won't be
    /// dropped. This method should be called after [`Self::next()`]
    #[cfg(feature = "overlapped-lists")]
    fn read_to_end(&mut self, name: QName) -> Result<(), DeError> {
        let mut depth = 0;
        loop {
            match self.read.pop_front() {
                Some(DeEvent::Start(e)) if e.name() == name => {
                    depth += 1;
                }
                Some(DeEvent::End(e)) if e.name() == name => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                }

                // Drop all other skipped events
                Some(_) => continue,

                // If we do not have skipped events, use effective reading that will
                // not allocate memory for events
                None => {
                    // We should close all opened tags, because we could buffer
                    // Start events, but not the corresponding End events. So we
                    // keep reading events until we exit all nested tags.
                    // `read_to_end()` will return an error if an Eof was encountered
                    // preliminary (in case of malformed XML).
                    //
                    // <tag><tag></tag></tag>
                    // ^^^^^^^^^^             - buffered in `self.read`, when `self.read_to_end()` is called, depth = 2
                    //           ^^^^^^       - read by the first call of `self.reader.read_to_end()`
                    //                 ^^^^^^ - read by the second call of `self.reader.read_to_end()`
                    loop {
                        self.reader.read_to_end(name)?;
                        if depth == 0 {
                            break;
                        }
                        depth -= 1;
                    }
                    break;
                }
            }
        }
        Ok(())
    }
    #[cfg(not(feature = "overlapped-lists"))]
    fn read_to_end(&mut self, name: QName) -> Result<(), DeError> {
        // First one might be in self.peek
        match self.next()? {
            DeEvent::Start(e) => self.reader.read_to_end(e.name())?,
            DeEvent::End(e) if e.name() == name => return Ok(()),
            _ => (),
        }
        self.reader.read_to_end(name)
    }

    fn skip_next_tree(&mut self) -> Result<(), DeError> {
        let DeEvent::Start(start) = self.next()? else {
            unreachable!("Only call this if the next event is a start event")
        };
        let name = start.name();
        self.read_to_end(name)
    }
}

impl<'de> Deserializer<'de, SliceReader<'de>> {
    /// Create new deserializer that will borrow data from the specified string.
    ///
    /// Deserializer created with this method will not resolve custom entities.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(source: &'de str) -> Self {
        Self::from_str_with_resolver(source, PredefinedEntityResolver)
    }
}

impl<'de, E> Deserializer<'de, SliceReader<'de>, E>
where
    E: EntityResolver,
{
    /// Create new deserializer that will borrow data from the specified string
    /// and use specified entity resolver.
    pub fn from_str_with_resolver(source: &'de str, entity_resolver: E) -> Self {
        let mut reader = NsReader::from_str(source);
        let config = reader.config_mut();
        config.expand_empty_elements = true;

        Self::new(
            SliceReader {
                reader,
                start_trimmer: StartTrimmer::default(),
            },
            entity_resolver,
        )
    }
}

impl<'de, R> Deserializer<'de, IoReader<R>>
where
    R: BufRead,
{
    /// Create new deserializer that will copy data from the specified reader
    /// into internal buffer.
    ///
    /// If you already have a string use [`Self::from_str`] instead, because it
    /// will borrow instead of copy. If you have `&[u8]` which is known to represent
    /// UTF-8, you can decode it first before using [`from_str`].
    ///
    /// Deserializer created with this method will not resolve custom entities.
    pub fn from_reader(reader: R) -> Self {
        Self::with_resolver(reader, PredefinedEntityResolver)
    }
}

impl<'de, R, E> Deserializer<'de, IoReader<R>, E>
where
    R: BufRead,
    E: EntityResolver,
{
    /// Create new deserializer that will copy data from the specified reader
    /// into internal buffer and use specified entity resolver.
    ///
    /// If you already have a string use [`Self::from_str`] instead, because it
    /// will borrow instead of copy. If you have `&[u8]` which is known to represent
    /// UTF-8, you can decode it first before using [`from_str`].
    pub fn with_resolver(reader: R, entity_resolver: E) -> Self {
        let mut reader = NsReader::from_reader(reader);
        let config = reader.config_mut();
        config.expand_empty_elements = true;

        Self::new(
            IoReader {
                reader,
                start_trimmer: StartTrimmer::default(),
                buf: Vec::new(),
            },
            entity_resolver,
        )
    }
}

impl<'de, 'a, R, E> de::Deserializer<'de> for &'a mut Deserializer<'de, R, E>
where
    R: XmlRead<'de>,
    E: EntityResolver,
{
    type Error = DeError;

    deserialize_primitives!();

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, DeError>
    where
        V: Visitor<'de>,
    {
        match self.next()? {
            DeEvent::Start(e) => visitor.visit_map(ElementMapAccess::new(self, e, fields)?),
            // SAFETY: The reader is guaranteed that we don't have unmatched tags
            // If we here, then out deserializer has a bug
            DeEvent::End(e) => unreachable!("{:?}", e),
            // Deserializer methods are only hints, if deserializer could not satisfy
            // request, it should return the data that it has. It is responsibility
            // of a Visitor to return an error if it does not understand the data
            DeEvent::Text(e) => match e.text {
                Cow::Borrowed(s) => visitor.visit_borrowed_str(s),
                Cow::Owned(s) => visitor.visit_string(s),
            },
            DeEvent::Eof => Err(DeError::UnexpectedEof),
        }
    }

    /// Unit represented in XML as a `xs:element` or text/CDATA content.
    /// Any content inside `xs:element` is ignored and skipped.
    ///
    /// Produces unit struct from any of following inputs:
    /// - any `<tag ...>...</tag>`
    /// - any `<tag .../>`
    /// - any consequent text / CDATA content (can consist of several parts
    ///   delimited by comments and processing instructions)
    ///
    /// # Events handling
    ///
    /// |Event             |XML                        |Handling
    /// |------------------|---------------------------|-------------------------------------------
    /// |[`DeEvent::Start`]|`<tag>...</tag>`           |Calls `visitor.visit_unit()`, consumes all events up to and including corresponding `End` event
    /// |[`DeEvent::End`]  |`</tag>`                   |This is impossible situation, the method will panic if it happens
    /// |[`DeEvent::Text`] |`text content` or `<![CDATA[cdata content]]>` (probably mixed)|Calls `visitor.visit_unit()`. The content is ignored
    /// |[`DeEvent::Eof`]  |                           |Emits [`UnexpectedEof`](DeError::UnexpectedEof)
    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, DeError>
    where
        V: Visitor<'de>,
    {
        match self.next()? {
            DeEvent::Start(s) => {
                self.read_to_end(s.name())?;
                visitor.visit_unit()
            }
            DeEvent::Text(_) => visitor.visit_unit(),
            // SAFETY: The reader is guaranteed that we don't have unmatched tags
            // If we here, then out deserializer has a bug
            DeEvent::End(e) => unreachable!("{:?}", e),
            DeEvent::Eof => Err(DeError::UnexpectedEof),
        }
    }

    /// Forwards deserialization of the inner type. Always calls [`Visitor::visit_newtype_struct`]
    /// with the same deserializer.
    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, DeError>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, DeError>
    where
        V: Visitor<'de>,
    {
        visitor.visit_enum(var::EnumAccess::new(self))
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, DeError>
    where
        V: Visitor<'de>,
    {
        visitor.visit_seq(self)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, DeError>
    where
        V: Visitor<'de>,
    {
        // We cannot use result of `peek()` directly because of borrow checker
        let _ = self.peek()?;
        match self.last_peeked() {
            DeEvent::Text(t) if t.is_empty() => visitor.visit_none(),
            DeEvent::Eof => visitor.visit_none(),
            // if the `xsi:nil` attribute is set to true we got a none value
            DeEvent::Start(start) if self.reader.reader.has_nil_attr(&start) => {
                self.skip_next_tree()?;
                visitor.visit_none()
            }
            _ => visitor.visit_some(self),
        }
    }

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, DeError>
    where
        V: Visitor<'de>,
    {
        match self.peek()? {
            DeEvent::Text(_) => self.deserialize_str(visitor),
            _ => self.deserialize_map(visitor),
        }
    }
}

/// An accessor to sequence elements forming a value for top-level sequence of XML
/// elements.
///
/// Technically, multiple top-level elements violates XML rule of only one top-level
/// element, but we consider this as several concatenated XML documents.
impl<'de, 'a, R, E> SeqAccess<'de> for &'a mut Deserializer<'de, R, E>
where
    R: XmlRead<'de>,
    E: EntityResolver,
{
    type Error = DeError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        match self.peek()? {
            DeEvent::Eof => {
                // We need to consume event in order to self.is_empty() worked
                self.next()?;
                Ok(None)
            }

            // Start(tag), End(tag), Text
            _ => seed.deserialize(&mut **self).map(Some),
        }
    }
}

impl<'de, 'a, R, E> IntoDeserializer<'de, DeError> for &'a mut Deserializer<'de, R, E>
where
    R: XmlRead<'de>,
    E: EntityResolver,
{
    type Deserializer = Self;

    #[inline]
    fn into_deserializer(self) -> Self {
        self
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// Helper struct that contains a state for an algorithm of converting events
/// from raw events to semi-trimmed events that is independent from a way of
/// events reading.
struct StartTrimmer {
    /// If `true`, then leading whitespace will be removed from next returned
    /// [`Event::Text`]. This field is set to `true` after reading each event
    /// except [`Event::Text`] and [`Event::CData`], so [`Event::Text`] events
    /// read right after them does not trimmed.
    trim_start: bool,
}

impl StartTrimmer {
    /// Converts raw reader's event into a payload event.
    /// Returns `None`, if event should be skipped.
    #[inline(always)]
    fn trim<'a>(&mut self, event: Event<'a>) -> Option<PayloadEvent<'a>> {
        let (event, trim_next_event) = match event {
            Event::DocType(e) => (PayloadEvent::DocType(e), true),
            Event::Start(e) => (PayloadEvent::Start(e), true),
            Event::End(e) => (PayloadEvent::End(e), true),
            Event::Eof => (PayloadEvent::Eof, true),

            // Do not trim next text event after Text or CDATA event
            Event::CData(e) => (PayloadEvent::CData(e), false),
            Event::Text(mut e) => {
                // If event is empty after trimming, skip it
                if self.trim_start && e.inplace_trim_start() {
                    return None;
                }
                (PayloadEvent::Text(e), false)
            }

            _ => return None,
        };
        self.trim_start = trim_next_event;
        Some(event)
    }
}

impl Default for StartTrimmer {
    #[inline]
    fn default() -> Self {
        Self { trim_start: true }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// Trait used by the deserializer for iterating over input. This is manually
/// "specialized" for iterating over `&[u8]`.
///
/// You do not need to implement this trait, it is needed to abstract from
/// [borrowing](SliceReader) and [copying](IoReader) data sources and reuse code in
/// deserializer
pub trait XmlRead<'i> {
    /// Return an input-borrowing event.
    fn next(&mut self) -> Result<PayloadEvent<'i>, DeError>;

    /// Skips until end element is found. Unlike `next()` it will not allocate
    /// when it cannot satisfy the lifetime.
    fn read_to_end(&mut self, name: QName) -> Result<(), DeError>;

    /// A copy of the reader's decoder used to decode strings.
    fn decoder(&self) -> Decoder;

    /// Checks if the `start` tag has a [`xsi:nil`] attribute. This method ignores
    /// any errors in attributes.
    ///
    /// [`xsi:nil`]: https://www.w3.org/TR/xmlschema-1/#xsi_nil
    fn has_nil_attr(&self, start: &BytesStart) -> bool;
}

/// XML input source that reads from a std::io input stream.
///
/// You cannot create it, it is created automatically when you call
/// [`Deserializer::from_reader`]
pub struct IoReader<R: BufRead> {
    reader: NsReader<R>,
    start_trimmer: StartTrimmer,
    buf: Vec<u8>,
}

impl<R: BufRead> IoReader<R> {
    /// Returns the underlying XML reader.
    ///
    /// ```
    /// # use pretty_assertions::assert_eq;
    /// use serde::Deserialize;
    /// use std::io::Cursor;
    /// use quick_xml::de::Deserializer;
    /// use quick_xml::NsReader;
    ///
    /// #[derive(Deserialize)]
    /// struct SomeStruct {
    ///     field1: String,
    ///     field2: String,
    /// }
    ///
    /// // Try to deserialize from broken XML
    /// let mut de = Deserializer::from_reader(Cursor::new(
    ///     "<SomeStruct><field1><field2></SomeStruct>"
    /// //   0                           ^= 28        ^= 41
    /// ));
    ///
    /// let err = SomeStruct::deserialize(&mut de);
    /// assert!(err.is_err());
    ///
    /// let reader: &NsReader<Cursor<&str>> = de.get_ref().get_ref();
    ///
    /// assert_eq!(reader.error_position(), 28);
    /// assert_eq!(reader.buffer_position(), 41);
    /// ```
    pub const fn get_ref(&self) -> &NsReader<R> {
        &self.reader
    }
}

impl<'i, R: BufRead> XmlRead<'i> for IoReader<R> {
    fn next(&mut self) -> Result<PayloadEvent<'static>, DeError> {
        loop {
            self.buf.clear();

            let event = self.reader.read_event_into(&mut self.buf)?;
            if let Some(event) = self.start_trimmer.trim(event) {
                return Ok(event.into_owned());
            }
        }
    }

    fn read_to_end(&mut self, name: QName) -> Result<(), DeError> {
        match self.reader.read_to_end_into(name, &mut self.buf) {
            Err(e) => Err(e.into()),
            Ok(_) => Ok(()),
        }
    }

    fn decoder(&self) -> Decoder {
        self.reader.decoder()
    }

    fn has_nil_attr(&self, start: &BytesStart) -> bool {
        start.attributes().has_nil(&self.reader)
    }
}

/// XML input source that reads from a slice of bytes and can borrow from it.
///
/// You cannot create it, it is created automatically when you call
/// [`Deserializer::from_str`].
pub struct SliceReader<'de> {
    reader: NsReader<&'de [u8]>,
    start_trimmer: StartTrimmer,
}

impl<'de> SliceReader<'de> {
    /// Returns the underlying XML reader.
    ///
    /// ```
    /// # use pretty_assertions::assert_eq;
    /// use serde::Deserialize;
    /// use quick_xml::de::Deserializer;
    /// use quick_xml::NsReader;
    ///
    /// #[derive(Deserialize)]
    /// struct SomeStruct {
    ///     field1: String,
    ///     field2: String,
    /// }
    ///
    /// // Try to deserialize from broken XML
    /// let mut de = Deserializer::from_str(
    ///     "<SomeStruct><field1><field2></SomeStruct>"
    /// //   0                           ^= 28        ^= 41
    /// );
    ///
    /// let err = SomeStruct::deserialize(&mut de);
    /// assert!(err.is_err());
    ///
    /// let reader: &NsReader<&[u8]> = de.get_ref().get_ref();
    ///
    /// assert_eq!(reader.error_position(), 28);
    /// assert_eq!(reader.buffer_position(), 41);
    /// ```
    pub const fn get_ref(&self) -> &NsReader<&'de [u8]> {
        &self.reader
    }
}

impl<'de> XmlRead<'de> for SliceReader<'de> {
    fn next(&mut self) -> Result<PayloadEvent<'de>, DeError> {
        loop {
            let event = self.reader.read_event()?;
            if let Some(event) = self.start_trimmer.trim(event) {
                return Ok(event);
            }
        }
    }

    fn read_to_end(&mut self, name: QName) -> Result<(), DeError> {
        match self.reader.read_to_end(name) {
            Err(e) => Err(e.into()),
            Ok(_) => Ok(()),
        }
    }

    fn decoder(&self) -> Decoder {
        self.reader.decoder()
    }

    fn has_nil_attr(&self, start: &BytesStart) -> bool {
        start.attributes().has_nil(&self.reader)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::IllFormedError;
    use pretty_assertions::assert_eq;

    fn make_de<'de>(source: &'de str) -> Deserializer<'de, SliceReader<'de>> {
        dbg!(source);
        Deserializer::from_str(source)
    }

    #[cfg(feature = "overlapped-lists")]
    mod skip {
        use super::*;
        use crate::de::DeEvent::*;
        use crate::events::BytesEnd;
        use pretty_assertions::assert_eq;

        /// Checks that `peek()` and `read()` behaves correctly after `skip()`
        #[test]
        fn read_and_peek() {
            let mut de = make_de(
                r#"
                <root>
                    <inner>
                        text
                        <inner/>
                    </inner>
                    <next/>
                    <target/>
                </root>
                "#,
            );

            // Initial conditions - both are empty
            assert_eq!(de.read, vec![]);
            assert_eq!(de.write, vec![]);

            assert_eq!(de.next().unwrap(), Start(BytesStart::new("root")));
            assert_eq!(de.peek().unwrap(), &Start(BytesStart::new("inner")));

            // Mark that start_replay() should begin replay from this point
            let checkpoint = de.skip_checkpoint();
            assert_eq!(checkpoint, 0);

            // Should skip first <inner> tree
            de.skip().unwrap();
            assert_eq!(de.read, vec![]);
            assert_eq!(
                de.write,
                vec![
                    Start(BytesStart::new("inner")),
                    Text("text".into()),
                    Start(BytesStart::new("inner")),
                    End(BytesEnd::new("inner")),
                    End(BytesEnd::new("inner")),
                ]
            );

            // Consume <next/>. Now unconsumed XML looks like:
            //
            //   <inner>
            //     text
            //     <inner/>
            //   </inner>
            //   <target/>
            // </root>
            assert_eq!(de.next().unwrap(), Start(BytesStart::new("next")));
            assert_eq!(de.next().unwrap(), End(BytesEnd::new("next")));

            // We finish writing. Next call to `next()` should start replay that messages:
            //
            //   <inner>
            //     text
            //     <inner/>
            //   </inner>
            //
            // and after that stream that messages:
            //
            //   <target/>
            // </root>
            de.start_replay(checkpoint);
            assert_eq!(
                de.read,
                vec![
                    Start(BytesStart::new("inner")),
                    Text("text".into()),
                    Start(BytesStart::new("inner")),
                    End(BytesEnd::new("inner")),
                    End(BytesEnd::new("inner")),
                ]
            );
            assert_eq!(de.write, vec![]);
            assert_eq!(de.next().unwrap(), Start(BytesStart::new("inner")));

            // Mark that start_replay() should begin replay from this point
            let checkpoint = de.skip_checkpoint();
            assert_eq!(checkpoint, 0);

            // Skip `$text` node and consume <inner/> after it
            de.skip().unwrap();
            assert_eq!(
                de.read,
                vec![
                    Start(BytesStart::new("inner")),
                    End(BytesEnd::new("inner")),
                    End(BytesEnd::new("inner")),
                ]
            );
            assert_eq!(
                de.write,
                vec![
                    // This comment here to keep the same formatting of both arrays
                    // otherwise rustfmt suggest one-line it
                    Text("text".into()),
                ]
            );

            assert_eq!(de.next().unwrap(), Start(BytesStart::new("inner")));
            assert_eq!(de.next().unwrap(), End(BytesEnd::new("inner")));

            // We finish writing. Next call to `next()` should start replay messages:
            //
            //     text
            //   </inner>
            //
            // and after that stream that messages:
            //
            //   <target/>
            // </root>
            de.start_replay(checkpoint);
            assert_eq!(
                de.read,
                vec![
                    // This comment here to keep the same formatting as others
                    // otherwise rustfmt suggest one-line it
                    Text("text".into()),
                    End(BytesEnd::new("inner")),
                ]
            );
            assert_eq!(de.write, vec![]);
            assert_eq!(de.next().unwrap(), Text("text".into()));
            assert_eq!(de.next().unwrap(), End(BytesEnd::new("inner")));
            assert_eq!(de.next().unwrap(), Start(BytesStart::new("target")));
            assert_eq!(de.next().unwrap(), End(BytesEnd::new("target")));
            assert_eq!(de.next().unwrap(), End(BytesEnd::new("root")));
            assert_eq!(de.next().unwrap(), Eof);
        }

        /// Checks that `read_to_end()` behaves correctly after `skip()`
        #[test]
        fn read_to_end() {
            let mut de = make_de(
                r#"
                <root>
                    <skip>
                        text
                        <skip/>
                    </skip>
                    <target>
                        <target/>
                    </target>
                </root>
                "#,
            );

            // Initial conditions - both are empty
            assert_eq!(de.read, vec![]);
            assert_eq!(de.write, vec![]);

            assert_eq!(de.next().unwrap(), Start(BytesStart::new("root")));

            // Mark that start_replay() should begin replay from this point
            let checkpoint = de.skip_checkpoint();
            assert_eq!(checkpoint, 0);

            // Skip the <skip> tree
            de.skip().unwrap();
            assert_eq!(de.read, vec![]);
            assert_eq!(
                de.write,
                vec![
                    Start(BytesStart::new("skip")),
                    Text("text".into()),
                    Start(BytesStart::new("skip")),
                    End(BytesEnd::new("skip")),
                    End(BytesEnd::new("skip")),
                ]
            );

            // Drop all events that represents <target> tree. Now unconsumed XML looks like:
            //
            //   <skip>
            //     text
            //     <skip/>
            //   </skip>
            // </root>
            assert_eq!(de.next().unwrap(), Start(BytesStart::new("target")));
            de.read_to_end(QName(b"target")).unwrap();
            assert_eq!(de.read, vec![]);
            assert_eq!(
                de.write,
                vec![
                    Start(BytesStart::new("skip")),
                    Text("text".into()),
                    Start(BytesStart::new("skip")),
                    End(BytesEnd::new("skip")),
                    End(BytesEnd::new("skip")),
                ]
            );

            // We finish writing. Next call to `next()` should start replay that messages:
            //
            //   <skip>
            //     text
            //     <skip/>
            //   </skip>
            //
            // and after that stream that messages:
            //
            // </root>
            de.start_replay(checkpoint);
            assert_eq!(
                de.read,
                vec![
                    Start(BytesStart::new("skip")),
                    Text("text".into()),
                    Start(BytesStart::new("skip")),
                    End(BytesEnd::new("skip")),
                    End(BytesEnd::new("skip")),
                ]
            );
            assert_eq!(de.write, vec![]);

            assert_eq!(de.next().unwrap(), Start(BytesStart::new("skip")));
            de.read_to_end(QName(b"skip")).unwrap();

            assert_eq!(de.next().unwrap(), End(BytesEnd::new("root")));
            assert_eq!(de.next().unwrap(), Eof);
        }

        /// Checks that replay replayes only part of events
        /// Test for https://github.com/tafia/quick-xml/issues/435
        #[test]
        fn partial_replay() {
            let mut de = make_de(
                r#"
                <root>
                    <skipped-1/>
                    <skipped-2/>
                    <inner>
                        <skipped-3/>
                        <skipped-4/>
                        <target-2/>
                    </inner>
                    <target-1/>
                </root>
                "#,
            );

            // Initial conditions - both are empty
            assert_eq!(de.read, vec![]);
            assert_eq!(de.write, vec![]);

            assert_eq!(de.next().unwrap(), Start(BytesStart::new("root")));

            // start_replay() should start replay from this point
            let checkpoint1 = de.skip_checkpoint();
            assert_eq!(checkpoint1, 0);

            // Should skip first and second <skipped-N/> elements
            de.skip().unwrap(); // skipped-1
            de.skip().unwrap(); // skipped-2
            assert_eq!(de.read, vec![]);
            assert_eq!(
                de.write,
                vec![
                    Start(BytesStart::new("skipped-1")),
                    End(BytesEnd::new("skipped-1")),
                    Start(BytesStart::new("skipped-2")),
                    End(BytesEnd::new("skipped-2")),
                ]
            );

            ////////////////////////////////////////////////////////////////////////////////////////

            assert_eq!(de.next().unwrap(), Start(BytesStart::new("inner")));
            assert_eq!(de.peek().unwrap(), &Start(BytesStart::new("skipped-3")));
            assert_eq!(
                de.read,
                vec![
                    // This comment here to keep the same formatting of both arrays
                    // otherwise rustfmt suggest one-line it
                    Start(BytesStart::new("skipped-3")),
                ]
            );
            assert_eq!(
                de.write,
                vec![
                    Start(BytesStart::new("skipped-1")),
                    End(BytesEnd::new("skipped-1")),
                    Start(BytesStart::new("skipped-2")),
                    End(BytesEnd::new("skipped-2")),
                ]
            );

            // start_replay() should start replay from this point
            let checkpoint2 = de.skip_checkpoint();
            assert_eq!(checkpoint2, 4);

            // Should skip third and forth <skipped-N/> elements
            de.skip().unwrap(); // skipped-3
            de.skip().unwrap(); // skipped-4
            assert_eq!(de.read, vec![]);
            assert_eq!(
                de.write,
                vec![
                    // checkpoint 1
                    Start(BytesStart::new("skipped-1")),
                    End(BytesEnd::new("skipped-1")),
                    Start(BytesStart::new("skipped-2")),
                    End(BytesEnd::new("skipped-2")),
                    // checkpoint 2
                    Start(BytesStart::new("skipped-3")),
                    End(BytesEnd::new("skipped-3")),
                    Start(BytesStart::new("skipped-4")),
                    End(BytesEnd::new("skipped-4")),
                ]
            );
            assert_eq!(de.next().unwrap(), Start(BytesStart::new("target-2")));
            assert_eq!(de.next().unwrap(), End(BytesEnd::new("target-2")));
            assert_eq!(de.peek().unwrap(), &End(BytesEnd::new("inner")));
            assert_eq!(
                de.read,
                vec![
                    // This comment here to keep the same formatting of both arrays
                    // otherwise rustfmt suggest one-line it
                    End(BytesEnd::new("inner")),
                ]
            );
            assert_eq!(
                de.write,
                vec![
                    // checkpoint 1
                    Start(BytesStart::new("skipped-1")),
                    End(BytesEnd::new("skipped-1")),
                    Start(BytesStart::new("skipped-2")),
                    End(BytesEnd::new("skipped-2")),
                    // checkpoint 2
                    Start(BytesStart::new("skipped-3")),
                    End(BytesEnd::new("skipped-3")),
                    Start(BytesStart::new("skipped-4")),
                    End(BytesEnd::new("skipped-4")),
                ]
            );

            // Start replay events from checkpoint 2
            de.start_replay(checkpoint2);
            assert_eq!(
                de.read,
                vec![
                    Start(BytesStart::new("skipped-3")),
                    End(BytesEnd::new("skipped-3")),
                    Start(BytesStart::new("skipped-4")),
                    End(BytesEnd::new("skipped-4")),
                    End(BytesEnd::new("inner")),
                ]
            );
            assert_eq!(
                de.write,
                vec![
                    Start(BytesStart::new("skipped-1")),
                    End(BytesEnd::new("skipped-1")),
                    Start(BytesStart::new("skipped-2")),
                    End(BytesEnd::new("skipped-2")),
                ]
            );

            // Replayed events
            assert_eq!(de.next().unwrap(), Start(BytesStart::new("skipped-3")));
            assert_eq!(de.next().unwrap(), End(BytesEnd::new("skipped-3")));
            assert_eq!(de.next().unwrap(), Start(BytesStart::new("skipped-4")));
            assert_eq!(de.next().unwrap(), End(BytesEnd::new("skipped-4")));

            assert_eq!(de.next().unwrap(), End(BytesEnd::new("inner")));
            assert_eq!(de.read, vec![]);
            assert_eq!(
                de.write,
                vec![
                    Start(BytesStart::new("skipped-1")),
                    End(BytesEnd::new("skipped-1")),
                    Start(BytesStart::new("skipped-2")),
                    End(BytesEnd::new("skipped-2")),
                ]
            );

            ////////////////////////////////////////////////////////////////////////////////////////

            // New events
            assert_eq!(de.next().unwrap(), Start(BytesStart::new("target-1")));
            assert_eq!(de.next().unwrap(), End(BytesEnd::new("target-1")));

            assert_eq!(de.read, vec![]);
            assert_eq!(
                de.write,
                vec![
                    Start(BytesStart::new("skipped-1")),
                    End(BytesEnd::new("skipped-1")),
                    Start(BytesStart::new("skipped-2")),
                    End(BytesEnd::new("skipped-2")),
                ]
            );

            // Start replay events from checkpoint 1
            de.start_replay(checkpoint1);
            assert_eq!(
                de.read,
                vec![
                    Start(BytesStart::new("skipped-1")),
                    End(BytesEnd::new("skipped-1")),
                    Start(BytesStart::new("skipped-2")),
                    End(BytesEnd::new("skipped-2")),
                ]
            );
            assert_eq!(de.write, vec![]);

            // Replayed events
            assert_eq!(de.next().unwrap(), Start(BytesStart::new("skipped-1")));
            assert_eq!(de.next().unwrap(), End(BytesEnd::new("skipped-1")));
            assert_eq!(de.next().unwrap(), Start(BytesStart::new("skipped-2")));
            assert_eq!(de.next().unwrap(), End(BytesEnd::new("skipped-2")));

            assert_eq!(de.read, vec![]);
            assert_eq!(de.write, vec![]);

            // New events
            assert_eq!(de.next().unwrap(), End(BytesEnd::new("root")));
            assert_eq!(de.next().unwrap(), Eof);
        }

        /// Checks that limiting buffer size works correctly
        #[test]
        fn limit() {
            use serde::Deserialize;

            #[derive(Debug, Deserialize)]
            #[allow(unused)]
            struct List {
                item: Vec<()>,
            }

            let mut de = make_de(
                r#"
                <any-name>
                    <item/>
                    <another-item>
                        <some-element>with text</some-element>
                        <yet-another-element/>
                    </another-item>
                    <item/>
                    <item/>
                </any-name>
                "#,
            );
            de.event_buffer_size(NonZeroUsize::new(3));

            match List::deserialize(&mut de) {
                Err(DeError::TooManyEvents(count)) => assert_eq!(count.get(), 3),
                e => panic!("Expected `Err(TooManyEvents(3))`, but got `{:?}`", e),
            }
        }

        /// Without handling Eof in `skip` this test failed with memory allocation
        #[test]
        fn invalid_xml() {
            use crate::de::DeEvent::*;

            let mut de = make_de("<root>");

            // Cache all events
            let checkpoint = de.skip_checkpoint();
            de.skip().unwrap();
            de.start_replay(checkpoint);
            assert_eq!(de.read, vec![Start(BytesStart::new("root")), Eof]);
        }
    }

    mod read_to_end {
        use super::*;
        use crate::de::DeEvent::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn complex() {
            let mut de = make_de(
                r#"
                <root>
                    <tag a="1"><tag>text</tag>content</tag>
                    <tag a="2"><![CDATA[cdata content]]></tag>
                    <self-closed/>
                </root>
                "#,
            );

            assert_eq!(de.next().unwrap(), Start(BytesStart::new("root")));

            assert_eq!(
                de.next().unwrap(),
                Start(BytesStart::from_content(r#"tag a="1""#, 3))
            );
            assert_eq!(de.read_to_end(QName(b"tag")).unwrap(), ());

            assert_eq!(
                de.next().unwrap(),
                Start(BytesStart::from_content(r#"tag a="2""#, 3))
            );
            assert_eq!(de.next().unwrap(), Text("cdata content".into()));
            assert_eq!(de.next().unwrap(), End(BytesEnd::new("tag")));

            assert_eq!(de.next().unwrap(), Start(BytesStart::new("self-closed")));
            assert_eq!(de.read_to_end(QName(b"self-closed")).unwrap(), ());

            assert_eq!(de.next().unwrap(), End(BytesEnd::new("root")));
            assert_eq!(de.next().unwrap(), Eof);
        }

        #[test]
        fn invalid_xml1() {
            let mut de = make_de("<tag><tag></tag>");

            assert_eq!(de.next().unwrap(), Start(BytesStart::new("tag")));
            assert_eq!(de.peek().unwrap(), &Start(BytesStart::new("tag")));

            match de.read_to_end(QName(b"tag")) {
                Err(DeError::InvalidXml(Error::IllFormed(cause))) => {
                    assert_eq!(cause, IllFormedError::MissingEndTag("tag".into()))
                }
                x => panic!(
                    "Expected `Err(InvalidXml(IllFormed(_)))`, but got `{:?}`",
                    x
                ),
            }
            assert_eq!(de.next().unwrap(), Eof);
        }

        #[test]
        fn invalid_xml2() {
            let mut de = make_de("<tag><![CDATA[]]><tag></tag>");

            assert_eq!(de.next().unwrap(), Start(BytesStart::new("tag")));
            assert_eq!(de.peek().unwrap(), &Text("".into()));

            match de.read_to_end(QName(b"tag")) {
                Err(DeError::InvalidXml(Error::IllFormed(cause))) => {
                    assert_eq!(cause, IllFormedError::MissingEndTag("tag".into()))
                }
                x => panic!(
                    "Expected `Err(InvalidXml(IllFormed(_)))`, but got `{:?}`",
                    x
                ),
            }
            assert_eq!(de.next().unwrap(), Eof);
        }
    }

    #[test]
    fn borrowing_reader_parity() {
        let s = r#"
            <item name="hello" source="world.rs">Some text</item>
            <item2/>
            <item3 value="world" />
        "#;

        let mut reader1 = IoReader {
            reader: NsReader::from_reader(s.as_bytes()),
            start_trimmer: StartTrimmer::default(),
            buf: Vec::new(),
        };
        let mut reader2 = SliceReader {
            reader: NsReader::from_str(s),
            start_trimmer: StartTrimmer::default(),
        };

        loop {
            let event1 = reader1.next().unwrap();
            let event2 = reader2.next().unwrap();

            if let (PayloadEvent::Eof, PayloadEvent::Eof) = (&event1, &event2) {
                break;
            }

            assert_eq!(event1, event2);
        }
    }

    #[test]
    fn borrowing_reader_events() {
        let s = r#"
            <item name="hello" source="world.rs">Some text</item>
            <item2></item2>
            <item3/>
            <item4 value="world" />
        "#;

        let mut reader = SliceReader {
            reader: NsReader::from_str(s),
            start_trimmer: StartTrimmer::default(),
        };

        let config = reader.reader.config_mut();
        config.expand_empty_elements = true;

        let mut events = Vec::new();

        loop {
            let event = reader.next().unwrap();
            if let PayloadEvent::Eof = event {
                break;
            }
            events.push(event);
        }

        use crate::de::PayloadEvent::*;

        assert_eq!(
            events,
            vec![
                Start(BytesStart::from_content(
                    r#"item name="hello" source="world.rs""#,
                    4
                )),
                Text(BytesText::from_escaped("Some text")),
                End(BytesEnd::new("item")),
                Start(BytesStart::from_content("item2", 5)),
                End(BytesEnd::new("item2")),
                Start(BytesStart::from_content("item3", 5)),
                End(BytesEnd::new("item3")),
                Start(BytesStart::from_content(r#"item4 value="world" "#, 5)),
                End(BytesEnd::new("item4")),
            ]
        )
    }

    /// Ensures, that [`Deserializer::read_string()`] never can get an `End` event,
    /// because parser reports error early
    #[test]
    fn read_string() {
        match from_str::<String>(r#"</root>"#) {
            Err(DeError::InvalidXml(Error::IllFormed(cause))) => {
                assert_eq!(cause, IllFormedError::UnmatchedEndTag("root".into()));
            }
            x => panic!(
                "Expected `Err(InvalidXml(IllFormed(_)))`, but got `{:?}`",
                x
            ),
        }

        let s: String = from_str(r#"<root></root>"#).unwrap();
        assert_eq!(s, "");

        match from_str::<String>(r#"<root></other>"#) {
            Err(DeError::InvalidXml(Error::IllFormed(cause))) => assert_eq!(
                cause,
                IllFormedError::MismatchedEndTag {
                    expected: "root".into(),
                    found: "other".into(),
                }
            ),
            x => panic!("Expected `Err(InvalidXml(IllFormed(_))`, but got `{:?}`", x),
        }
    }

    /// Tests for https://github.com/tafia/quick-xml/issues/474.
    ///
    /// That tests ensures that comments and processed instructions is ignored
    /// and can split one logical string in pieces.
    mod merge_text {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn text() {
            let mut de = make_de("text");
            assert_eq!(de.next().unwrap(), DeEvent::Text("text".into()));
        }

        #[test]
        fn cdata() {
            let mut de = make_de("<![CDATA[cdata]]>");
            assert_eq!(de.next().unwrap(), DeEvent::Text("cdata".into()));
        }

        #[test]
        fn text_and_cdata() {
            let mut de = make_de("text and <![CDATA[cdata]]>");
            assert_eq!(de.next().unwrap(), DeEvent::Text("text and cdata".into()));
        }

        #[test]
        fn text_and_empty_cdata() {
            let mut de = make_de("text and <![CDATA[]]>");
            assert_eq!(de.next().unwrap(), DeEvent::Text("text and ".into()));
        }

        #[test]
        fn cdata_and_text() {
            let mut de = make_de("<![CDATA[cdata]]> and text");
            assert_eq!(de.next().unwrap(), DeEvent::Text("cdata and text".into()));
        }

        #[test]
        fn empty_cdata_and_text() {
            let mut de = make_de("<![CDATA[]]> and text");
            assert_eq!(de.next().unwrap(), DeEvent::Text(" and text".into()));
        }

        #[test]
        fn cdata_and_cdata() {
            let mut de = make_de(
                "\
                    <![CDATA[cdata]]]]>\
                    <![CDATA[>cdata]]>\
                ",
            );
            assert_eq!(de.next().unwrap(), DeEvent::Text("cdata]]>cdata".into()));
        }

        mod comment_between {
            use super::*;
            use pretty_assertions::assert_eq;

            #[test]
            fn text() {
                let mut de = make_de(
                    "\
                        text \
                        <!--comment 1--><!--comment 2--> \
                        text\
                    ",
                );
                assert_eq!(de.next().unwrap(), DeEvent::Text("text  text".into()));
            }

            #[test]
            fn cdata() {
                let mut de = make_de(
                    "\
                        <![CDATA[cdata]]]]>\
                        <!--comment 1--><!--comment 2-->\
                        <![CDATA[>cdata]]>\
                    ",
                );
                assert_eq!(de.next().unwrap(), DeEvent::Text("cdata]]>cdata".into()));
            }

            #[test]
            fn text_and_cdata() {
                let mut de = make_de(
                    "\
                        text \
                        <!--comment 1--><!--comment 2-->\
                        <![CDATA[ cdata]]>\
                    ",
                );
                assert_eq!(de.next().unwrap(), DeEvent::Text("text  cdata".into()));
            }

            #[test]
            fn text_and_empty_cdata() {
                let mut de = make_de(
                    "\
                        text \
                        <!--comment 1--><!--comment 2-->\
                        <![CDATA[]]>\
                    ",
                );
                assert_eq!(de.next().unwrap(), DeEvent::Text("text ".into()));
            }

            #[test]
            fn cdata_and_text() {
                let mut de = make_de(
                    "\
                        <![CDATA[cdata ]]>\
                        <!--comment 1--><!--comment 2--> \
                        text \
                    ",
                );
                assert_eq!(de.next().unwrap(), DeEvent::Text("cdata  text".into()));
            }

            #[test]
            fn empty_cdata_and_text() {
                let mut de = make_de(
                    "\
                        <![CDATA[]]>\
                        <!--comment 1--><!--comment 2--> \
                        text \
                    ",
                );
                assert_eq!(de.next().unwrap(), DeEvent::Text(" text".into()));
            }

            #[test]
            fn cdata_and_cdata() {
                let mut de = make_de(
                    "\
                        <![CDATA[cdata]]]>\
                        <!--comment 1--><!--comment 2-->\
                        <![CDATA[]>cdata]]>\
                    ",
                );
                assert_eq!(de.next().unwrap(), DeEvent::Text("cdata]]>cdata".into()));
            }
        }

        mod pi_between {
            use super::*;
            use pretty_assertions::assert_eq;

            #[test]
            fn text() {
                let mut de = make_de(
                    "\
                        text \
                        <?pi 1?><?pi 2?> \
                        text\
                    ",
                );
                assert_eq!(de.next().unwrap(), DeEvent::Text("text  text".into()));
            }

            #[test]
            fn cdata() {
                let mut de = make_de(
                    "\
                        <![CDATA[cdata]]]]>\
                        <?pi 1?><?pi 2?>\
                        <![CDATA[>cdata]]>\
                    ",
                );
                assert_eq!(de.next().unwrap(), DeEvent::Text("cdata]]>cdata".into()));
            }

            #[test]
            fn text_and_cdata() {
                let mut de = make_de(
                    "\
                        text \
                        <?pi 1?><?pi 2?>\
                        <![CDATA[ cdata]]>\
                    ",
                );
                assert_eq!(de.next().unwrap(), DeEvent::Text("text  cdata".into()));
            }

            #[test]
            fn text_and_empty_cdata() {
                let mut de = make_de(
                    "\
                        text \
                        <?pi 1?><?pi 2?>\
                        <![CDATA[]]>\
                    ",
                );
                assert_eq!(de.next().unwrap(), DeEvent::Text("text ".into()));
            }

            #[test]
            fn cdata_and_text() {
                let mut de = make_de(
                    "\
                        <![CDATA[cdata ]]>\
                        <?pi 1?><?pi 2?> \
                        text \
                    ",
                );
                assert_eq!(de.next().unwrap(), DeEvent::Text("cdata  text".into()));
            }

            #[test]
            fn empty_cdata_and_text() {
                let mut de = make_de(
                    "\
                        <![CDATA[]]>\
                        <?pi 1?><?pi 2?> \
                        text \
                    ",
                );
                assert_eq!(de.next().unwrap(), DeEvent::Text(" text".into()));
            }

            #[test]
            fn cdata_and_cdata() {
                let mut de = make_de(
                    "\
                        <![CDATA[cdata]]]>\
                        <?pi 1?><?pi 2?>\
                        <![CDATA[]>cdata]]>\
                    ",
                );
                assert_eq!(de.next().unwrap(), DeEvent::Text("cdata]]>cdata".into()));
            }
        }
    }

    /// Tests for https://github.com/tafia/quick-xml/issues/474.
    ///
    /// This tests ensures that any combination of payload data is processed
    /// as expected.
    mod triples {
        use super::*;
        use pretty_assertions::assert_eq;

        mod start {
            use super::*;

            /// <tag1><tag2>...
            mod start {
                use super::*;
                use pretty_assertions::assert_eq;

                #[test]
                fn start() {
                    let mut de = make_de("<tag1><tag2><tag3>");
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag1")));
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag2")));
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag3")));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                /// Not matching end tag will result to error
                #[test]
                fn end() {
                    let mut de = make_de("<tag1><tag2></tag2>");
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag1")));
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag2")));
                    assert_eq!(de.next().unwrap(), DeEvent::End(BytesEnd::new("tag2")));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                #[test]
                fn text() {
                    let mut de = make_de("<tag1><tag2> text ");
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag1")));
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag2")));
                    // Text is trimmed from both sides
                    assert_eq!(de.next().unwrap(), DeEvent::Text("text".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                #[test]
                fn cdata() {
                    let mut de = make_de("<tag1><tag2><![CDATA[ cdata ]]>");
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag1")));
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag2")));
                    assert_eq!(de.next().unwrap(), DeEvent::Text(" cdata ".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                #[test]
                fn eof() {
                    let mut de = make_de("<tag1><tag2>");
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag1")));
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag2")));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }
            }

            /// <tag></tag>...
            mod end {
                use super::*;
                use pretty_assertions::assert_eq;

                #[test]
                fn start() {
                    let mut de = make_de("<tag></tag><tag2>");
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag")));
                    assert_eq!(de.next().unwrap(), DeEvent::End(BytesEnd::new("tag")));
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag2")));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                #[test]
                fn end() {
                    let mut de = make_de("<tag></tag></tag2>");
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag")));
                    assert_eq!(de.next().unwrap(), DeEvent::End(BytesEnd::new("tag")));
                    match de.next() {
                        Err(DeError::InvalidXml(Error::IllFormed(cause))) => {
                            assert_eq!(cause, IllFormedError::UnmatchedEndTag("tag2".into()));
                        }
                        x => panic!(
                            "Expected `Err(InvalidXml(IllFormed(_)))`, but got `{:?}`",
                            x
                        ),
                    }
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                #[test]
                fn text() {
                    let mut de = make_de("<tag></tag> text ");
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag")));
                    assert_eq!(de.next().unwrap(), DeEvent::End(BytesEnd::new("tag")));
                    // Text is trimmed from both sides
                    assert_eq!(de.next().unwrap(), DeEvent::Text("text".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                #[test]
                fn cdata() {
                    let mut de = make_de("<tag></tag><![CDATA[ cdata ]]>");
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag")));
                    assert_eq!(de.next().unwrap(), DeEvent::End(BytesEnd::new("tag")));
                    assert_eq!(de.next().unwrap(), DeEvent::Text(" cdata ".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                #[test]
                fn eof() {
                    let mut de = make_de("<tag></tag>");
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag")));
                    assert_eq!(de.next().unwrap(), DeEvent::End(BytesEnd::new("tag")));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }
            }

            /// <tag> text ...
            mod text {
                use super::*;
                use pretty_assertions::assert_eq;

                #[test]
                fn start() {
                    let mut de = make_de("<tag> text <tag2>");
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag")));
                    // Text is trimmed from both sides
                    assert_eq!(de.next().unwrap(), DeEvent::Text("text".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag2")));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                #[test]
                fn end() {
                    let mut de = make_de("<tag> text </tag>");
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag")));
                    // Text is trimmed from both sides
                    assert_eq!(de.next().unwrap(), DeEvent::Text("text".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::End(BytesEnd::new("tag")));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                // start::text::text has no difference from start::text

                #[test]
                fn cdata() {
                    let mut de = make_de("<tag> text <![CDATA[ cdata ]]>");
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag")));
                    // Text is trimmed from the start
                    assert_eq!(de.next().unwrap(), DeEvent::Text("text  cdata ".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                #[test]
                fn eof() {
                    let mut de = make_de("<tag> text ");
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag")));
                    // Text is trimmed from both sides
                    assert_eq!(de.next().unwrap(), DeEvent::Text("text".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }
            }

            /// <tag><![CDATA[ cdata ]]>...
            mod cdata {
                use super::*;
                use pretty_assertions::assert_eq;

                #[test]
                fn start() {
                    let mut de = make_de("<tag><![CDATA[ cdata ]]><tag2>");
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag")));
                    assert_eq!(de.next().unwrap(), DeEvent::Text(" cdata ".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag2")));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                #[test]
                fn end() {
                    let mut de = make_de("<tag><![CDATA[ cdata ]]></tag>");
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag")));
                    assert_eq!(de.next().unwrap(), DeEvent::Text(" cdata ".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::End(BytesEnd::new("tag")));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                #[test]
                fn text() {
                    let mut de = make_de("<tag><![CDATA[ cdata ]]> text ");
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag")));
                    // Text is trimmed from the end
                    assert_eq!(de.next().unwrap(), DeEvent::Text(" cdata  text".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                #[test]
                fn cdata() {
                    let mut de = make_de("<tag><![CDATA[ cdata ]]><![CDATA[ cdata2 ]]>");
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag")));
                    assert_eq!(de.next().unwrap(), DeEvent::Text(" cdata  cdata2 ".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                #[test]
                fn eof() {
                    let mut de = make_de("<tag><![CDATA[ cdata ]]>");
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag")));
                    assert_eq!(de.next().unwrap(), DeEvent::Text(" cdata ".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }
            }
        }

        /// Start from End event will always generate an error
        #[test]
        fn end() {
            let mut de = make_de("</tag>");
            match de.next() {
                Err(DeError::InvalidXml(Error::IllFormed(cause))) => {
                    assert_eq!(cause, IllFormedError::UnmatchedEndTag("tag".into()));
                }
                x => panic!(
                    "Expected `Err(InvalidXml(IllFormed(_)))`, but got `{:?}`",
                    x
                ),
            }
            assert_eq!(de.next().unwrap(), DeEvent::Eof);
        }

        mod text {
            use super::*;
            use pretty_assertions::assert_eq;

            mod start {
                use super::*;
                use pretty_assertions::assert_eq;

                #[test]
                fn start() {
                    let mut de = make_de(" text <tag1><tag2>");
                    // Text is trimmed from both sides
                    assert_eq!(de.next().unwrap(), DeEvent::Text("text".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag1")));
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag2")));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                /// Not matching end tag will result in error
                #[test]
                fn end() {
                    let mut de = make_de(" text <tag></tag>");
                    // Text is trimmed from both sides
                    assert_eq!(de.next().unwrap(), DeEvent::Text("text".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag")));
                    assert_eq!(de.next().unwrap(), DeEvent::End(BytesEnd::new("tag")));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                #[test]
                fn text() {
                    let mut de = make_de(" text <tag> text2 ");
                    // Text is trimmed from both sides
                    assert_eq!(de.next().unwrap(), DeEvent::Text("text".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag")));
                    // Text is trimmed from both sides
                    assert_eq!(de.next().unwrap(), DeEvent::Text("text2".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                #[test]
                fn cdata() {
                    let mut de = make_de(" text <tag><![CDATA[ cdata ]]>");
                    // Text is trimmed from both sides
                    assert_eq!(de.next().unwrap(), DeEvent::Text("text".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag")));
                    assert_eq!(de.next().unwrap(), DeEvent::Text(" cdata ".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                #[test]
                fn eof() {
                    // Text is trimmed from both sides
                    let mut de = make_de(" text <tag>");
                    assert_eq!(de.next().unwrap(), DeEvent::Text("text".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag")));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }
            }

            /// End event without corresponding start event will always generate an error
            #[test]
            fn end() {
                let mut de = make_de(" text </tag>");
                // Text is trimmed from both sides
                assert_eq!(de.next().unwrap(), DeEvent::Text("text".into()));
                match de.next() {
                    Err(DeError::InvalidXml(Error::IllFormed(cause))) => {
                        assert_eq!(cause, IllFormedError::UnmatchedEndTag("tag".into()));
                    }
                    x => panic!(
                        "Expected `Err(InvalidXml(IllFormed(_)))`, but got `{:?}`",
                        x
                    ),
                }
                assert_eq!(de.next().unwrap(), DeEvent::Eof);
            }

            // text::text::something is equivalent to text::something

            mod cdata {
                use super::*;
                use pretty_assertions::assert_eq;

                #[test]
                fn start() {
                    let mut de = make_de(" text <![CDATA[ cdata ]]><tag>");
                    // Text is trimmed from the start
                    assert_eq!(de.next().unwrap(), DeEvent::Text("text  cdata ".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag")));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                #[test]
                fn end() {
                    let mut de = make_de(" text <![CDATA[ cdata ]]></tag>");
                    // Text is trimmed from the start
                    assert_eq!(de.next().unwrap(), DeEvent::Text("text  cdata ".into()));
                    match de.next() {
                        Err(DeError::InvalidXml(Error::IllFormed(cause))) => {
                            assert_eq!(cause, IllFormedError::UnmatchedEndTag("tag".into()));
                        }
                        x => panic!(
                            "Expected `Err(InvalidXml(IllFormed(_)))`, but got `{:?}`",
                            x
                        ),
                    }
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                #[test]
                fn text() {
                    let mut de = make_de(" text <![CDATA[ cdata ]]> text2 ");
                    // Text is trimmed from the start and from the end
                    assert_eq!(
                        de.next().unwrap(),
                        DeEvent::Text("text  cdata  text2".into())
                    );
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                #[test]
                fn cdata() {
                    let mut de = make_de(" text <![CDATA[ cdata ]]><![CDATA[ cdata2 ]]>");
                    // Text is trimmed from the start
                    assert_eq!(
                        de.next().unwrap(),
                        DeEvent::Text("text  cdata  cdata2 ".into())
                    );
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                #[test]
                fn eof() {
                    let mut de = make_de(" text <![CDATA[ cdata ]]>");
                    // Text is trimmed from the start
                    assert_eq!(de.next().unwrap(), DeEvent::Text("text  cdata ".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }
            }
        }

        mod cdata {
            use super::*;
            use pretty_assertions::assert_eq;

            mod start {
                use super::*;
                use pretty_assertions::assert_eq;

                #[test]
                fn start() {
                    let mut de = make_de("<![CDATA[ cdata ]]><tag1><tag2>");
                    assert_eq!(de.next().unwrap(), DeEvent::Text(" cdata ".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag1")));
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag2")));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                /// Not matching end tag will result in error
                #[test]
                fn end() {
                    let mut de = make_de("<![CDATA[ cdata ]]><tag></tag>");
                    assert_eq!(de.next().unwrap(), DeEvent::Text(" cdata ".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag")));
                    assert_eq!(de.next().unwrap(), DeEvent::End(BytesEnd::new("tag")));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                #[test]
                fn text() {
                    let mut de = make_de("<![CDATA[ cdata ]]><tag> text ");
                    assert_eq!(de.next().unwrap(), DeEvent::Text(" cdata ".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag")));
                    // Text is trimmed from both sides
                    assert_eq!(de.next().unwrap(), DeEvent::Text("text".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                #[test]
                fn cdata() {
                    let mut de = make_de("<![CDATA[ cdata ]]><tag><![CDATA[ cdata2 ]]>");
                    assert_eq!(de.next().unwrap(), DeEvent::Text(" cdata ".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag")));
                    assert_eq!(de.next().unwrap(), DeEvent::Text(" cdata2 ".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                #[test]
                fn eof() {
                    let mut de = make_de("<![CDATA[ cdata ]]><tag>");
                    assert_eq!(de.next().unwrap(), DeEvent::Text(" cdata ".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag")));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }
            }

            /// End event without corresponding start event will always generate an error
            #[test]
            fn end() {
                let mut de = make_de("<![CDATA[ cdata ]]></tag>");
                assert_eq!(de.next().unwrap(), DeEvent::Text(" cdata ".into()));
                match de.next() {
                    Err(DeError::InvalidXml(Error::IllFormed(cause))) => {
                        assert_eq!(cause, IllFormedError::UnmatchedEndTag("tag".into()));
                    }
                    x => panic!(
                        "Expected `Err(InvalidXml(IllFormed(_)))`, but got `{:?}`",
                        x
                    ),
                }
                assert_eq!(de.next().unwrap(), DeEvent::Eof);
            }

            mod text {
                use super::*;
                use pretty_assertions::assert_eq;

                #[test]
                fn start() {
                    let mut de = make_de("<![CDATA[ cdata ]]> text <tag>");
                    // Text is trimmed from the end
                    assert_eq!(de.next().unwrap(), DeEvent::Text(" cdata  text".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag")));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                #[test]
                fn end() {
                    let mut de = make_de("<![CDATA[ cdata ]]> text </tag>");
                    // Text is trimmed from the end
                    assert_eq!(de.next().unwrap(), DeEvent::Text(" cdata  text".into()));
                    match de.next() {
                        Err(DeError::InvalidXml(Error::IllFormed(cause))) => {
                            assert_eq!(cause, IllFormedError::UnmatchedEndTag("tag".into()));
                        }
                        x => panic!(
                            "Expected `Err(InvalidXml(IllFormed(_)))`, but got `{:?}`",
                            x
                        ),
                    }
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                // cdata::text::text is equivalent to cdata::text

                #[test]
                fn cdata() {
                    let mut de = make_de("<![CDATA[ cdata ]]> text <![CDATA[ cdata2 ]]>");
                    assert_eq!(
                        de.next().unwrap(),
                        DeEvent::Text(" cdata  text  cdata2 ".into())
                    );
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                #[test]
                fn eof() {
                    let mut de = make_de("<![CDATA[ cdata ]]> text ");
                    // Text is trimmed from the end
                    assert_eq!(de.next().unwrap(), DeEvent::Text(" cdata  text".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }
            }

            mod cdata {
                use super::*;
                use pretty_assertions::assert_eq;

                #[test]
                fn start() {
                    let mut de = make_de("<![CDATA[ cdata ]]><![CDATA[ cdata2 ]]><tag>");
                    assert_eq!(de.next().unwrap(), DeEvent::Text(" cdata  cdata2 ".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Start(BytesStart::new("tag")));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                #[test]
                fn end() {
                    let mut de = make_de("<![CDATA[ cdata ]]><![CDATA[ cdata2 ]]></tag>");
                    assert_eq!(de.next().unwrap(), DeEvent::Text(" cdata  cdata2 ".into()));
                    match de.next() {
                        Err(DeError::InvalidXml(Error::IllFormed(cause))) => {
                            assert_eq!(cause, IllFormedError::UnmatchedEndTag("tag".into()));
                        }
                        x => panic!(
                            "Expected `Err(InvalidXml(IllFormed(_)))`, but got `{:?}`",
                            x
                        ),
                    }
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                #[test]
                fn text() {
                    let mut de = make_de("<![CDATA[ cdata ]]><![CDATA[ cdata2 ]]> text ");
                    // Text is trimmed from the end
                    assert_eq!(
                        de.next().unwrap(),
                        DeEvent::Text(" cdata  cdata2  text".into())
                    );
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                #[test]
                fn cdata() {
                    let mut de =
                        make_de("<![CDATA[ cdata ]]><![CDATA[ cdata2 ]]><![CDATA[ cdata3 ]]>");
                    assert_eq!(
                        de.next().unwrap(),
                        DeEvent::Text(" cdata  cdata2  cdata3 ".into())
                    );
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }

                #[test]
                fn eof() {
                    let mut de = make_de("<![CDATA[ cdata ]]><![CDATA[ cdata2 ]]>");
                    assert_eq!(de.next().unwrap(), DeEvent::Text(" cdata  cdata2 ".into()));
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                    assert_eq!(de.next().unwrap(), DeEvent::Eof);
                }
            }
        }
    }
}
