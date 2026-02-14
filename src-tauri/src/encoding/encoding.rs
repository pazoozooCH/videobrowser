use base64::{engine::general_purpose::STANDARD, Engine};

pub fn encode_string(input: &str) -> String {
    STANDARD.encode(input.as_bytes())
}

pub fn decode_string(input: &str) -> Option<String> {
    STANDARD
        .decode(input)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_known_value() {
        assert_eq!(encode_string("Test"), "VGVzdA==");
    }

    #[test]
    fn test_decode_known_value() {
        assert_eq!(decode_string("VGVzdA=="), Some("Test".to_string()));
    }

    #[test]
    fn test_encode_special_chars() {
        assert_eq!(encode_string("éàè!+ç%&/^¨w "), "w6nDoMOoISvDpyUmL17CqHcg");
    }

    #[test]
    fn test_decode_special_chars() {
        assert_eq!(
            decode_string("w6nDoMOoISvDpyUmL17CqHcg"),
            Some("éàè!+ç%&/^¨w ".to_string())
        );
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let original = "Hello World";
        let encoded = encode_string(original);
        let decoded = decode_string(&encoded);
        assert_eq!(decoded, Some(original.to_string()));
    }

    #[test]
    fn test_roundtrip_special_chars() {
        let original = "éàè!+ç%&/^¨w ";
        let encoded = encode_string(original);
        let decoded = decode_string(&encoded);
        assert_eq!(decoded, Some(original.to_string()));
    }

    #[test]
    fn test_encode_empty_string() {
        let encoded = encode_string("");
        assert_eq!(encoded, "");
        assert_eq!(decode_string(&encoded), Some("".to_string()));
    }

    #[test]
    fn test_encode_unicode() {
        let original = "日本語テスト";
        let encoded = encode_string(original);
        let decoded = decode_string(&encoded);
        assert_eq!(decoded, Some(original.to_string()));
    }

    #[test]
    fn test_decode_invalid() {
        assert_eq!(decode_string("not valid base64!!!"), None);
    }

    #[test]
    fn test_decode_invalid_utf8() {
        // Valid base64 but decodes to invalid UTF-8
        let encoded = STANDARD.encode(&[0xFF, 0xFE]);
        assert_eq!(decode_string(&encoded), None);
    }
}
