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

    #[test]
    fn test_try_decode_encoded_name() {
        let encoded = format!(".dat_{}", encode_string("my_folder"));
        assert_eq!(try_decode_name(&encoded), Some("my_folder".to_string()));
    }

    #[test]
    fn test_try_decode_plain_name() {
        assert_eq!(try_decode_name("regular_folder"), None);
    }

    #[test]
    fn test_encode_name() {
        let result = encode_name("test");
        assert!(result.starts_with(".dat_"));
    }
}
