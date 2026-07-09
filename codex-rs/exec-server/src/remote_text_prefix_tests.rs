use pretty_assertions::assert_eq;

use super::*;

#[test]
fn rejects_malformed_cardinality_base64_and_utf8() {
    for response in [
        FsReadTextPrefixesBatchResponse {
            results: Vec::new(),
        },
        data_response("not-base64"),
        data_response("/w=="),
    ] {
        let error = decode_response(response, 1, 4).expect_err("malformed response");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
}

fn data_response(data_base64: &str) -> FsReadTextPrefixesBatchResponse {
    FsReadTextPrefixesBatchResponse {
        results: vec![FsReadTextPrefixesBatchResult::Data {
            data_base64: data_base64.to_string(),
            complete: false,
        }],
    }
}
