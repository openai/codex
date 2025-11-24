use displaydoc::Display;

#[derive(Debug, Display)]
pub enum DataStoreError {
    /// data store disconnected
    Disconnect,
    /// the data for key `{0}` is not available
    Redaction(String),
    /// invalid header (expected {expected:?}, found {found:?})
    InvalidHeader { expected: String, found: String },
    /// unknown data store error
    Unknown,
}

fn main() {
    let disconnect = DataStoreError::Disconnect;
    println!(
        "Enum value `Disconnect` should be printed as:\n\t{}",
        disconnect
    );

    let redaction = DataStoreError::Redaction(String::from("Dummy"));
    println!(
        "Enum value `Redaction` should be printed as:\n\t{}",
        redaction
    );

    let invalid_header = DataStoreError::InvalidHeader {
        expected: String::from("https"),
        found: String::from("http"),
    };
    println!(
        "Enum value `InvalidHeader` should be printed as:\n\t{}",
        invalid_header
    );
}
