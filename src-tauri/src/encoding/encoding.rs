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
    fn test_encode_decode_roundtrip() {
        let original = "Hello World";
        let encoded = encode_string(original);
        let decoded = decode_string(&encoded);
        assert_eq!(decoded, Some(original.to_string()));
    }

    #[test]
    fn test_decode_invalid() {
        assert_eq!(decode_string("not valid base64!!!"), None);
    }
}
