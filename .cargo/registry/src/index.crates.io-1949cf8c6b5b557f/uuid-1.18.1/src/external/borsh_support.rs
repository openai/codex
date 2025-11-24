#[cfg(test)]
mod borsh_tests {
    use crate::Uuid;
    use std::string::ToString;

    #[test]
    fn test_serialize() {
        let uuid_str = "f9168c5e-ceb2-4faa-b6bf-329bf39fa1e4";
        let uuid = Uuid::parse_str(uuid_str).unwrap();
        let uuid_bytes = uuid.as_bytes().to_vec();
        let borsh_bytes = borsh::to_vec(&uuid).unwrap();
        assert_eq!(uuid_bytes, borsh_bytes);
    }

    #[test]
    fn test_deserialize() {
        let uuid_str = "f9168c5e-ceb2-4faa-b6bf-329bf39fa1e4";
        let uuid = Uuid::parse_str(uuid_str).unwrap();
        let uuid_bytes = uuid.as_bytes().to_vec();
        let deserialized = borsh::from_slice::<Uuid>(&uuid_bytes).unwrap().to_string();
        assert_eq!(uuid_str, deserialized);
    }
}
