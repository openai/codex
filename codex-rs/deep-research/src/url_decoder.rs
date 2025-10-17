// DuckDuckGo URL Decoder
// Decodes DuckDuckGo redirect URLs to actual URLs

/// DuckDuckGoのリダイレクトURLから実際のURLを抽出
/// 例: //duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com → https://example.com
pub fn decode_duckduckgo_url(url: &str) -> String {
    // DuckDuckGoのリダイレクトURLかチェック
    if url.contains("duckduckgo.com/l/?uddg=") {
        // uddgパラメータを抽出
        if let Some(start_idx) = url.find("uddg=") {
            let encoded = &url[start_idx + 5..];
            // &amp;以降を削除
            let encoded = if let Some(amp_idx) = encoded.find("&amp;") {
                &encoded[..amp_idx]
            } else {
                encoded
            };

            // URLデコード
            match urlencoding::decode(encoded) {
                Ok(decoded) => {
                    eprintln!("🔗 [DEBUG] Decoded URL: {} -> {}", url, decoded);
                    return decoded.to_string();
                }
                Err(e) => {
                    eprintln!("⚠️  [WARNING] Failed to decode URL: {}", e);
                }
            }
        }
    }

    // デコード失敗または通常のURLの場合はそのまま返す
    url.to_string()
}

/// URLリストを一括デコード
#[allow(dead_code)]
pub fn decode_urls(urls: Vec<String>) -> Vec<String> {
    urls.into_iter()
        .map(|url| decode_duckduckgo_url(&url))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_decode_duckduckgo_url() {
        let redirect_url =
            "//duckduckgo.com/l/?uddg=https%3A%2F%2Fdoc.rust-lang.org%2Fbook&amp;rut=abc123";
        let decoded = decode_duckduckgo_url(redirect_url);
        assert_eq!(decoded, "https://doc.rust-lang.org/book");
    }

    #[test]
    fn test_decode_normal_url() {
        let normal_url = "https://example.com/page";
        let decoded = decode_duckduckgo_url(normal_url);
        assert_eq!(decoded, normal_url);
    }

    #[test]
    fn test_decode_urls_batch() {
        let urls = vec![
            "//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2F1&amp;rut=abc".to_string(),
            "https://normal-url.com".to_string(),
            "//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2F2&amp;rut=def".to_string(),
        ];

        let decoded = decode_urls(urls);

        assert_eq!(decoded[0], "https://example.com/1");
        assert_eq!(decoded[1], "https://normal-url.com");
        assert_eq!(decoded[2], "https://example.com/2");
    }
}
