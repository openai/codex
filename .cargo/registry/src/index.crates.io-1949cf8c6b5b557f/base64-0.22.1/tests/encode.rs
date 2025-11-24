use base64::{
    alphabet::URL_SAFE, engine::general_purpose::PAD, engine::general_purpose::STANDARD, *,
};

fn compare_encode(expected: &str, target: &[u8]) {
    assert_eq!(expected, STANDARD.encode(target));
}

#[test]
fn encode_all_ascii() {
    let ascii: Vec<u8> = (0..=127).collect();

    compare_encode(
        "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8gISIjJCUmJygpKissLS4vMDEyMzQ1Njc4OTo7P\
         D0+P0BBQkNERUZHSElKS0xNTk9QUVJTVFVWV1hZWltcXV5fYGFiY2RlZmdoaWprbG1ub3BxcnN0dXZ3eHl6e3x9fn8\
         =",
        &ascii,
    );
}

#[test]
fn encode_all_bytes() {
    let bytes: Vec<u8> = (0..=255).collect();

    compare_encode(
        "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8gISIjJCUmJygpKissLS4vMDEyMzQ1Njc4OTo7P\
         D0+P0BBQkNERUZHSElKS0xNTk9QUVJTVFVWV1hZWltcXV5fYGFiY2RlZmdoaWprbG1ub3BxcnN0dXZ3eHl6e3x9fn\
         +AgYKDhIWGh4iJiouMjY6PkJGSk5SVlpeYmZqbnJ2en6ChoqOkpaanqKmqq6ytrq+wsbKztLW2t7i5uru8vb6\
         /wMHCw8TFxsfIycrLzM3Oz9DR0tPU1dbX2Nna29zd3t/g4eLj5OXm5+jp6uvs7e7v8PHy8/T19vf4+fr7/P3+/w==",
        &bytes,
    );
}

#[test]
fn encode_all_bytes_url() {
    let bytes: Vec<u8> = (0..=255).collect();

    assert_eq!(
        "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8gISIjJCUmJygpKissLS4vMDEyMzQ1Njc4OTo7PD0\
         -P0BBQkNERUZHSElKS0xNTk9QUVJTVFVWV1hZWltcXV5fYGFiY2RlZmdoaWprbG1ub3BxcnN0dXZ3eHl6e3x9fn\
         -AgYKDhIWGh4iJiouMjY6PkJGSk5SVlpeYmZqbnJ2en6ChoqOkpaanqKmqq6ytrq\
         -wsbKztLW2t7i5uru8vb6_wMHCw8TFxsfIycrLzM3Oz9DR0tPU1dbX2Nna29zd3t_g4eLj5OXm5-jp6uvs7e7v8PHy\
         8_T19vf4-fr7_P3-_w==",
        &engine::GeneralPurpose::new(&URL_SAFE, PAD).encode(bytes)
    );
}

#[test]
fn encoded_len_unpadded() {
    assert_eq!(0, encoded_len(0, false).unwrap());
    assert_eq!(2, encoded_len(1, false).unwrap());
    assert_eq!(3, encoded_len(2, false).unwrap());
    assert_eq!(4, encoded_len(3, false).unwrap());
    assert_eq!(6, encoded_len(4, false).unwrap());
    assert_eq!(7, encoded_len(5, false).unwrap());
    assert_eq!(8, encoded_len(6, false).unwrap());
    assert_eq!(10, encoded_len(7, false).unwrap());
}

#[test]
fn encoded_len_padded() {
    assert_eq!(0, encoded_len(0, true).unwrap());
    assert_eq!(4, encoded_len(1, true).unwrap());
    assert_eq!(4, encoded_len(2, true).unwrap());
    assert_eq!(4, encoded_len(3, true).unwrap());
    assert_eq!(8, encoded_len(4, true).unwrap());
    assert_eq!(8, encoded_len(5, true).unwrap());
    assert_eq!(8, encoded_len(6, true).unwrap());
    assert_eq!(12, encoded_len(7, true).unwrap());
}
#[test]
fn encoded_len_overflow() {
    let max_size = usize::MAX / 4 * 3 + 2;
    assert_eq!(2, max_size % 3);
    assert_eq!(Some(usize::MAX), encoded_len(max_size, false));
    assert_eq!(None, encoded_len(max_size + 1, false));
}
