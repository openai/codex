#[cfg(feature = "serde")]
mod tests {
    use std::convert::TryInto;

    use pulldown_cmark::CowStr;

    #[test]
    fn cow_str_to_str_round_trip_bincode() {
        for i in &[
            CowStr::Borrowed("dssa"),
            CowStr::Borrowed(""),
            CowStr::Boxed("hello".to_owned().into_boxed_str()),
            CowStr::Boxed("".to_owned().into_boxed_str()),
            CowStr::Inlined("inline".try_into().unwrap()),
            CowStr::Inlined("".try_into().unwrap()),
        ] {
            let encoded = bincode::serialize(i).unwrap();
            let decoded1: CowStr = bincode::deserialize(&encoded).unwrap();
            let decoded2: String = bincode::deserialize(&encoded).unwrap();
            let decoded3: &str = bincode::deserialize(&encoded).unwrap();

            assert_eq!(&decoded1, i);
            assert_eq!(decoded2, i.as_ref());
            assert_eq!(decoded3, i.as_ref());
        }
    }

    #[test]
    fn cow_str_to_str_round_trip_json() {
        for i in &[
            CowStr::Borrowed("dssa"),
            CowStr::Borrowed(""),
            CowStr::Boxed("hello".to_owned().into_boxed_str()),
            CowStr::Boxed("".to_owned().into_boxed_str()),
            CowStr::Inlined("inline".try_into().unwrap()),
            CowStr::Inlined("".try_into().unwrap()),
        ] {
            let encoded = serde_json::to_string(i).unwrap();
            let decoded1: CowStr = serde_json::from_str(&encoded).unwrap();
            let decoded2: String = serde_json::from_str(&encoded).unwrap();
            let decoded3: &str = serde_json::from_str(&encoded).unwrap();

            assert_eq!(&decoded1, i);
            assert_eq!(decoded2, i.as_ref());
            assert_eq!(decoded3, i.as_ref());
        }
    }

    #[test]
    fn str_to_cow_str_json() {
        let str = "a borrowed str";
        let string = "a owned str".to_owned();

        let encoded_str = serde_json::to_string(&str).unwrap();
        let encoded_string = serde_json::to_string(&string).unwrap();

        let decoded_str: CowStr = serde_json::from_str(&encoded_str).unwrap();
        let decoded_string: CowStr = serde_json::from_str(&encoded_string).unwrap();

        assert_eq!(decoded_str.as_ref(), str);
        assert_eq!(decoded_string.as_ref(), string);
    }

    #[test]
    fn str_to_cow_str_bincode() {
        let str = "a borrowed str";
        let string = "a owned str".to_owned();

        let encoded_str = bincode::serialize(&str).unwrap();
        let encoded_string = bincode::serialize(&string).unwrap();

        let decoded_str: CowStr = bincode::deserialize(&encoded_str).unwrap();
        let decoded_string: CowStr = bincode::deserialize(&encoded_string).unwrap();

        assert_eq!(decoded_str.as_ref(), str);
        assert_eq!(decoded_string.as_ref(), string);
    }
}
