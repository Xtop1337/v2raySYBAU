use base64::alphabet;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use base64::Engine;

const LENIENT: GeneralPurposeConfig =
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent).with_decode_allow_trailing_bits(true);
const STANDARD_LENIENT: GeneralPurpose = GeneralPurpose::new(&alphabet::STANDARD, LENIENT);

/// Decodes standard or URL-safe base64 with or without padding, ignoring whitespace.
pub fn b64_decode(input: &str) -> Option<Vec<u8>> {
    let cleaned: String = input
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| match c {
            '-' => '+',
            '_' => '/',
            other => other,
        })
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    STANDARD_LENIENT.decode(cleaned.trim_end_matches('=')).ok()
}

pub fn b64_decode_str(input: &str) -> Option<String> {
    b64_decode(input).and_then(|bytes| String::from_utf8(bytes).ok())
}

pub fn b64_encode(input: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(input)
}

pub fn b64_encode_url(input: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(input)
}

pub fn percent_decode(value: &str) -> String {
    percent_encoding::percent_decode_str(value).decode_utf8_lossy().into_owned()
}

/// Characters that must be escaped inside a query value or fragment of a share link.
const COMPONENT: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC.remove(b'-').remove(b'_').remove(b'.').remove(b'~');

pub fn percent_encode(value: &str) -> String {
    percent_encoding::utf8_percent_encode(value, COMPONENT).to_string()
}

/// Deep-merges `patch` into `target`: objects are merged recursively, everything else is replaced.
pub fn merge_json(target: &mut serde_json::Value, patch: &serde_json::Value) {
    match (target, patch) {
        (serde_json::Value::Object(target), serde_json::Value::Object(patch)) => {
            for (key, value) in patch {
                merge_json(target.entry(key.clone()).or_insert(serde_json::Value::Null), value);
            }
        }
        (target, patch) => *target = patch.clone(),
    }
}

pub fn split_list(value: &str) -> Vec<String> {
    value.split([',', '\n', ';']).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_all_base64_flavours() {
        assert_eq!(b64_decode_str("aGVsbG8=").unwrap(), "hello");
        assert_eq!(b64_decode_str("aGVsbG8").unwrap(), "hello");
        assert_eq!(b64_decode_str("aGVs\nbG8=").unwrap(), "hello");
        assert_eq!(b64_decode_str("-_8").map(|s| s.len()), None);
        assert_eq!(b64_decode("-_8").unwrap(), vec![0xfb, 0xff]);
        assert!(b64_decode("vless://x").is_none());
    }

    #[test]
    fn merges_json() {
        let mut a = serde_json::json!({"log": {"loglevel": "warning", "access": "a"}, "x": 1});
        merge_json(&mut a, &serde_json::json!({"log": {"loglevel": "debug"}, "x": [1]}));
        assert_eq!(a, serde_json::json!({"log": {"loglevel": "debug", "access": "a"}, "x": [1]}));
    }
}
