use std::path::Path;

use super::encoding::{decode_string, encode_string};

const ENCODING_PREFIX: &str = ".dat_";

/// Returns the decoded name if the filename starts with the encoding prefix,
/// or None if it's not encoded.
pub fn try_decode_name(physical_name: &str) -> Option<String> {
    physical_name
        .strip_prefix(ENCODING_PREFIX)
        .and_then(decode_string)
}

/// Encodes a plain name into the `.dat_` prefixed format.
pub fn encode_name(name: &str) -> String {
    format!("{}{}", ENCODING_PREFIX, encode_string(name))
}

/// Checks whether encoding this path would produce a path shorter than 256 chars.
pub fn can_encode(path: &Path) -> bool {
    let physical_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();

    let encoded_name = encode_name(physical_name);
    let parent = path.parent().unwrap_or(path);
    let new_path = parent.join(&encoded_name);

    new_path.to_string_lossy().len() < 256
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn test_try_decode_encoded_name() {
        assert_eq!(
            try_decode_name(".dat_VGVzdA=="),
            Some("Test".to_string())
        );
    }

    #[test]
    fn test_try_decode_plain_name() {
        assert_eq!(try_decode_name("regular_folder"), None);
    }

    #[test]
    fn test_try_decode_not_base64_after_prefix() {
        assert_eq!(try_decode_name(".dat_!!!invalid!!!"), None);
    }

    #[test]
    fn test_encode_name_known_value() {
        assert_eq!(encode_name("Test"), ".dat_VGVzdA==");
    }

    #[test]
    fn test_encode_name_special_chars() {
        let encoded = encode_name("éàè!+ç%&/^¨w ");
        assert_eq!(encoded, ".dat_w6nDoMOoISvDpyUmL17CqHcg");
    }

    #[test]
    fn test_encode_decode_name_roundtrip() {
        let original = "my_folder";
        let encoded = encode_name(original);
        let decoded = try_decode_name(&encoded);
        assert_eq!(decoded, Some(original.to_string()));
    }

    #[test]
    fn test_encode_decode_name_roundtrip_unicode() {
        let original = "日本語フォルダ";
        let encoded = encode_name(original);
        let decoded = try_decode_name(&encoded);
        assert_eq!(decoded, Some(original.to_string()));
    }

    #[test]
    fn test_can_encode_short_path() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("short_name.txt");
        std::fs::write(&file, "").unwrap();
        assert!(can_encode(&file));
    }

    #[test]
    fn test_can_encode_long_path() {
        // Create a path that would exceed 256 chars when encoded
        let long_name = "a".repeat(200);
        let path = PathBuf::from("/tmp").join(&long_name);
        assert!(!can_encode(&path));
    }
}
