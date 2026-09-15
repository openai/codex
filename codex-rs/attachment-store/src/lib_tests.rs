use pretty_assertions::assert_eq;

use super::*;

/// Debug output redacts attachment bytes in requests and results.
#[test]
fn attachment_debug_output_redacts_bytes() {
    let request = UploadRequest {
        file_name: Some("image.png".to_string()),
        data: b"secret".to_vec(),
    };
    let result = UploadResult::Inline {
        bytes: b"secret".to_vec(),
    };

    assert_eq!(
        (format!("{request:?}"), format!("{result:?}")),
        (
            r#"UploadRequest { file_name: Some("image.png"), data: "<redacted>" }"#.to_string(),
            r#"Inline { bytes: "<redacted>" }"#.to_string(),
        )
    );
}

/// Debug output redacts credential-bearing file URLs in resolved metadata.
#[test]
fn attachment_metadata_debug_output_redacts_file_url() {
    let metadata = AttachmentMetadata {
        file_url: Some("https://attachments.test/file?signed=secret".to_string()),
        ..AttachmentMetadata::default()
    };

    let debug = format!("{metadata:?}");

    assert!(!debug.contains("signed=secret"));
    assert!(debug.contains("<redacted>"));
}

/// The inline store preserves PNG and JPEG attachment bytes.
#[tokio::test]
async fn inline_store_preserves_image_bytes() {
    let cases: [(&str, &[u8]); 2] = [
        ("image.png", b"\x89PNG\r\n\x1a\n"),
        ("image.jpg", b"\xff\xd8\xff\xe0JFIF\x00\xff\xd9"),
    ];

    for (file_name, data) in cases {
        let attachment = InlineAttachmentStore
            .upload(UploadRequest {
                file_name: Some(file_name.to_string()),
                data: data.to_vec(),
            })
            .await
            .expect("inline attachment");
        let UploadResult::Inline { bytes } = attachment else {
            panic!("inline store returned a file reference");
        };

        assert_eq!(bytes, data);
    }
}

/// The inline store reports file references as missing because it never uploads them.
#[tokio::test]
async fn inline_store_cannot_resolve_file_references() {
    let error = InlineAttachmentStore
        .resolve(ResolveRequest {
            file_id: "file_123",
            download_url_ttl: None,
        })
        .await
        .expect_err("inline store cannot resolve file references");

    assert_eq!(error.kind(), AttachmentStoreErrorKind::NotFound);
}
