#![cfg(test)]

use super::*;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use codex_app_server_protocol::FsReadFileResponse;
use codex_app_server_protocol::RequestId;
use pretty_assertions::assert_eq;

#[test]
fn file_read_writes_the_same_json_line_directly() {
    let mut output = BufWriter::with_capacity(FILE_RESPONSE_BUFFER_SIZE, Vec::new());
    let mut expected = Vec::new();
    for len in [0, 1, 2, FILE_RESPONSE_BUFFER_SIZE + 1] {
        let data: Vec<_> = (1..=u8::MAX).cycle().take(len).collect();
        let id = RequestId::String("id\"\n".to_string());
        let eager = OutgoingResponse {
            id: id.clone(),
            result: FsReadFileResponse {
                data_base64: STANDARD.encode(&data),
            },
        };
        serde_json::to_writer(&mut expected, &eager).expect("serialize eager response");
        expected.push(b'\n');

        let message = OutgoingMessage::Response(OutgoingResponse {
            id,
            result: Box::new(ClientResponsePayload::FsReadFileBytes(FsReadFileBytes {
                data,
            })),
        });
        let message = StdoutMessage::new(message).expect("file response");
        assert!(matches!(message, StdoutMessage::FileRead(_)));
        message.write_to(&mut output).expect("write file response");
        assert_eq!(output.get_ref(), &expected);
    }
}
