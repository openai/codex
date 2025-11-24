use super::*;
use std::net::IpAddr;

#[test]
fn test_map_as_tuple_list() {
    let ip = "1.2.3.4".parse().unwrap();
    let ip2 = "255.255.255.255".parse().unwrap();

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct SM(#[serde_as(as = "Seq<(DisplayFromStr, DisplayFromStr)>")] BTreeMap<u32, IpAddr>);

    let map: BTreeMap<_, _> = vec![(1, ip), (10, ip), (200, ip2)].into_iter().collect();
    is_equal(
        SM(map),
        expect![[r#"
            [
              [
                "1",
                "1.2.3.4"
              ],
              [
                "10",
                "1.2.3.4"
              ],
              [
                "200",
                "255.255.255.255"
              ]
            ]"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct SB(#[serde_as(as = "Vec<(DisplayFromStr, DisplayFromStr)>")] BTreeMap<u32, IpAddr>);

    let map: BTreeMap<_, _> = vec![(1, ip), (10, ip), (200, ip2)].into_iter().collect();
    is_equal(
        SB(map.clone()),
        expect![[r#"
            [
              [
                "1",
                "1.2.3.4"
              ],
              [
                "10",
                "1.2.3.4"
              ],
              [
                "200",
                "255.255.255.255"
              ]
            ]"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct SB2(#[serde_as(as = "Vec<(Same, DisplayFromStr)>")] BTreeMap<u32, IpAddr>);

    is_equal(
        SB2(map),
        expect![[r#"
            [
              [
                1,
                "1.2.3.4"
              ],
              [
                10,
                "1.2.3.4"
              ],
              [
                200,
                "255.255.255.255"
              ]
            ]"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct SH(#[serde_as(as = "Vec<(DisplayFromStr, DisplayFromStr)>")] HashMap<u32, IpAddr>);

    // HashMap serialization tests with more than 1 entry are unreliable
    let map1: HashMap<_, _> = vec![(200, ip2)].into_iter().collect();
    let map: HashMap<_, _> = vec![(1, ip), (10, ip), (200, ip2)].into_iter().collect();
    is_equal(
        SH(map1.clone()),
        expect![[r#"
            [
              [
                "200",
                "255.255.255.255"
              ]
            ]"#]],
    );
    check_deserialization(
        SH(map.clone()),
        r#"[["1","1.2.3.4"],["10","1.2.3.4"],["200","255.255.255.255"]]"#,
    );
    check_error_deserialization::<SH>(
        r#"{"200":"255.255.255.255"}"#,
        expect![[r#"invalid type: map, expected a sequence at line 1 column 0"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct SH2(#[serde_as(as = "Vec<(Same, DisplayFromStr)>")] HashMap<u32, IpAddr>);

    is_equal(
        SH2(map1),
        expect![[r#"
            [
              [
                200,
                "255.255.255.255"
              ]
            ]"#]],
    );
    check_deserialization(
        SH2(map),
        r#"[[1,"1.2.3.4"],[10,"1.2.3.4"],[200,"255.255.255.255"]]"#,
    );
    check_error_deserialization::<SH2>(
        r#"1"#,
        expect![[r#"invalid type: integer `1`, expected a sequence at line 1 column 1"#]],
    );
}

#[test]
fn test_tuple_list_as_map() {
    let ip = "1.2.3.4".parse().unwrap();
    let ip2 = "255.255.255.255".parse().unwrap();

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct SM(#[serde_as(as = "Map<DisplayFromStr, DisplayFromStr>")] Vec<(u32, IpAddr)>);

    is_equal(
        SM(vec![(1, ip), (10, ip), (200, ip2)]),
        expect![[r#"
            {
              "1": "1.2.3.4",
              "10": "1.2.3.4",
              "200": "255.255.255.255"
            }"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct SH(#[serde_as(as = "HashMap<DisplayFromStr, DisplayFromStr>")] Vec<(u32, IpAddr)>);

    is_equal(
        SH(vec![(1, ip), (10, ip), (200, ip2)]),
        expect![[r#"
            {
              "1": "1.2.3.4",
              "10": "1.2.3.4",
              "200": "255.255.255.255"
            }"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct SB(#[serde_as(as = "BTreeMap<DisplayFromStr, DisplayFromStr>")] Vec<(u32, IpAddr)>);

    is_equal(
        SB(vec![(1, ip), (10, ip), (200, ip2)]),
        expect![[r#"
            {
              "1": "1.2.3.4",
              "10": "1.2.3.4",
              "200": "255.255.255.255"
            }"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct SD(#[serde_as(as = "BTreeMap<DisplayFromStr, DisplayFromStr>")] VecDeque<(u32, IpAddr)>);

    is_equal(
        SD(vec![(1, ip), (10, ip), (200, ip2)].into()),
        expect![[r#"
            {
              "1": "1.2.3.4",
              "10": "1.2.3.4",
              "200": "255.255.255.255"
            }"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Sll(
        #[serde_as(as = "HashMap<DisplayFromStr, DisplayFromStr>")] LinkedList<(u32, IpAddr)>,
    );

    is_equal(
        Sll(vec![(1, ip), (10, ip), (200, ip2)].into_iter().collect()),
        expect![[r#"
            {
              "1": "1.2.3.4",
              "10": "1.2.3.4",
              "200": "255.255.255.255"
            }"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct SO(#[serde_as(as = "HashMap<DisplayFromStr, DisplayFromStr>")] Option<(u32, IpAddr)>);

    is_equal(
        SO(Some((1, ip))),
        expect![[r#"
            {
              "1": "1.2.3.4"
            }"#]],
    );
    is_equal(SO(None), expect![[r#"{}"#]]);
}

#[test]
fn test_tuple_array_as_map() {
    #[serde_as]
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct S0(#[serde_as(as = "Map<_, _>")] [(u8, u8); 1]);
    is_equal(
        S0([(1, 2)]),
        expect![[r#"
          {
            "1": 2
          }"#]],
    );

    #[serde_as]
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct S1(#[serde_as(as = "BTreeMap<_, _>")] [(u8, u8); 1]);
    is_equal(
        S1([(1, 2)]),
        expect![[r#"
          {
            "1": 2
          }"#]],
    );

    #[serde_as]
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct S2(#[serde_as(as = "HashMap<_, _>")] [(u8, u8); 33]);
    is_equal(
        S2([
            (0, 0),
            (1, 1),
            (2, 2),
            (3, 3),
            (4, 4),
            (5, 5),
            (6, 6),
            (7, 7),
            (8, 8),
            (9, 9),
            (10, 10),
            (11, 11),
            (12, 12),
            (13, 13),
            (14, 14),
            (15, 15),
            (16, 16),
            (17, 17),
            (18, 18),
            (19, 19),
            (20, 20),
            (21, 21),
            (22, 22),
            (23, 23),
            (24, 24),
            (25, 25),
            (26, 26),
            (27, 27),
            (28, 28),
            (29, 29),
            (30, 30),
            (31, 31),
            (32, 32),
        ]),
        expect![[r#"
            {
              "0": 0,
              "1": 1,
              "2": 2,
              "3": 3,
              "4": 4,
              "5": 5,
              "6": 6,
              "7": 7,
              "8": 8,
              "9": 9,
              "10": 10,
              "11": 11,
              "12": 12,
              "13": 13,
              "14": 14,
              "15": 15,
              "16": 16,
              "17": 17,
              "18": 18,
              "19": 19,
              "20": 20,
              "21": 21,
              "22": 22,
              "23": 23,
              "24": 24,
              "25": 25,
              "26": 26,
              "27": 27,
              "28": 28,
              "29": 29,
              "30": 30,
              "31": 31,
              "32": 32
            }"#]],
    );
}

// Test that the `Seq` conversion works when the inner type is explicitly specified.
#[test]
fn test_map_as_tuple_with_nested_complex_type() {
    #[serde_as]
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct S0(#[serde_as(as = "Seq<(_, Vec<_>)>")] BTreeMap<usize, Vec<usize>>);

    let value = S0(BTreeMap::from([
        (1, Vec::from([1, 2])),
        (2, Vec::from([3, 4])),
    ]));
    is_equal(
        value,
        expect![[r#"
            [
              [
                1,
                [
                  1,
                  2
                ]
              ],
              [
                2,
                [
                  3,
                  4
                ]
              ]
            ]"#]],
    );
}

// Problematic handling around fundamental types: https://github.com/rust-lang/rust/issues/121621
#[allow(unknown_lints, non_local_definitions)]
#[test]
fn test_map_as_tuple_list_works_with_serializer_that_needs_length_to_serialize_sequence() {
    use serde::{
        ser::{
            SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
            SerializeTupleStruct, SerializeTupleVariant,
        },
        Serializer,
    };
    use std::fmt::{self, Debug, Display};

    #[derive(Debug)]
    enum TestError {
        LengthRequired,
        Other,
    }

    impl Display for TestError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            Debug::fmt(self, f)
        }
    }

    impl std::error::Error for TestError {}

    impl serde::ser::Error for TestError {
        fn custom<T>(_: T) -> Self
        where
            T: Display,
        {
            TestError::Other
        }
    }

    struct TestSerializer;

    impl Serializer for &mut TestSerializer {
        type Ok = ();
        type Error = TestError;
        type SerializeSeq = Self;
        type SerializeTuple = Self;
        type SerializeTupleStruct = Self;
        type SerializeTupleVariant = Self;
        type SerializeMap = Self;
        type SerializeStruct = Self;
        type SerializeStructVariant = Self;

        fn serialize_bool(self, _: bool) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }

        fn serialize_i8(self, _: i8) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }

        fn serialize_i16(self, _: i16) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }

        fn serialize_i32(self, _: i32) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }

        fn serialize_i64(self, _: i64) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }

        fn serialize_u8(self, _: u8) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }

        fn serialize_u16(self, _: u16) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }

        fn serialize_u32(self, _: u32) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }

        fn serialize_u64(self, _: u64) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }

        fn serialize_f32(self, _: f32) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }

        fn serialize_f64(self, _: f64) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }

        fn serialize_char(self, _: char) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }

        fn serialize_str(self, _: &str) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }

        fn serialize_bytes(self, _: &[u8]) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }

        fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }

        fn serialize_some<T>(self, v: &T) -> Result<Self::Ok, Self::Error>
        where
            T: Serialize + ?Sized,
        {
            v.serialize(self)
        }

        fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }

        fn serialize_unit_struct(self, _: &'static str) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }

        fn serialize_unit_variant(
            self,
            _: &'static str,
            _: u32,
            _: &'static str,
        ) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }

        fn serialize_newtype_struct<T>(
            self,
            _: &'static str,
            value: &T,
        ) -> Result<Self::Ok, Self::Error>
        where
            T: Serialize + ?Sized,
        {
            value.serialize(self)
        }

        fn serialize_newtype_variant<T>(
            self,
            _: &'static str,
            _: u32,
            _: &'static str,
            value: &T,
        ) -> Result<Self::Ok, Self::Error>
        where
            T: Serialize + ?Sized,
        {
            value.serialize(self)
        }

        fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
            len.map(|_| self).ok_or(TestError::LengthRequired)
        }

        fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, Self::Error> {
            Ok(self)
        }

        fn serialize_tuple_struct(
            self,
            _: &'static str,
            _: usize,
        ) -> Result<Self::SerializeTupleStruct, Self::Error> {
            Ok(self)
        }

        fn serialize_tuple_variant(
            self,
            _: &'static str,
            _: u32,
            _: &'static str,
            _: usize,
        ) -> Result<Self::SerializeTupleVariant, Self::Error> {
            Ok(self)
        }

        fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
            Ok(self)
        }

        fn serialize_struct(
            self,
            _: &'static str,
            _: usize,
        ) -> Result<Self::SerializeStruct, Self::Error> {
            Ok(self)
        }

        fn serialize_struct_variant(
            self,
            _: &'static str,
            _: u32,
            _: &'static str,
            _: usize,
        ) -> Result<Self::SerializeStructVariant, Self::Error> {
            Ok(self)
        }
    }

    impl SerializeMap for &mut TestSerializer {
        type Ok = ();
        type Error = TestError;

        fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
        where
            T: Serialize + ?Sized,
        {
            key.serialize(&mut **self)
        }

        fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
        where
            T: Serialize + ?Sized,
        {
            value.serialize(&mut **self)
        }

        fn end(self) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }
    }

    impl SerializeSeq for &mut TestSerializer {
        type Ok = ();
        type Error = TestError;

        fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
        where
            T: Serialize + ?Sized,
        {
            value.serialize(&mut **self)
        }

        fn end(self) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }
    }

    impl SerializeStruct for &mut TestSerializer {
        type Ok = ();
        type Error = TestError;

        fn serialize_field<T>(&mut self, _: &'static str, value: &T) -> Result<(), Self::Error>
        where
            T: Serialize + ?Sized,
        {
            value.serialize(&mut **self)
        }

        fn end(self) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }
    }

    impl SerializeStructVariant for &mut TestSerializer {
        type Ok = ();
        type Error = TestError;

        fn serialize_field<T>(&mut self, _: &'static str, value: &T) -> Result<(), Self::Error>
        where
            T: Serialize + ?Sized,
        {
            value.serialize(&mut **self)
        }

        fn end(self) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }
    }

    impl SerializeTuple for &mut TestSerializer {
        type Ok = ();
        type Error = TestError;

        fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
        where
            T: Serialize + ?Sized,
        {
            value.serialize(&mut **self)
        }

        fn end(self) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }
    }

    impl SerializeTupleStruct for &mut TestSerializer {
        type Ok = ();
        type Error = TestError;

        fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
        where
            T: Serialize + ?Sized,
        {
            value.serialize(&mut **self)
        }

        fn end(self) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }
    }

    impl SerializeTupleVariant for &mut TestSerializer {
        type Ok = ();
        type Error = TestError;

        fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
        where
            T: Serialize + ?Sized,
        {
            value.serialize(&mut **self)
        }

        fn end(self) -> Result<Self::Ok, Self::Error> {
            Ok(())
        }
    }

    #[serde_as]
    #[derive(Debug, Default, Serialize)]
    struct Data {
        #[serde_as(as = "Seq<(_, _)>")]
        xs: HashMap<i32, i32>,
    }

    Data::default().serialize(&mut TestSerializer).unwrap();
}
