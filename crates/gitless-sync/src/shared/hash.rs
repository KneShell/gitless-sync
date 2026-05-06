use sha1::{Digest, Sha1};

#[must_use]
pub fn blob_hash(content: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(format!("blob {}\0", content.len()).as_bytes());
    hasher.update(content);
    let result = hasher.finalize();
    hex_encode(&result)
}

fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::normalize::normalize_text;

    #[test]
    fn empty_blob_matches_git() {
        // git's empty blob SHA-1 is the well-known constant
        assert_eq!(blob_hash(&[]), "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391");
    }

    #[test]
    fn crlf_normalizes_to_lf() {
        let crlf = normalize_text(b"hello\r\n", false);
        assert_eq!(blob_hash(&crlf), blob_hash(b"hello\n"));
    }

    #[test]
    fn same_binary_same_sha() {
        let bytes = [0u8, 1, 2, 3, 4, 5];
        assert_eq!(blob_hash(&bytes), blob_hash(&bytes));
    }

    #[test]
    fn different_content_different_sha() {
        assert_ne!(blob_hash(b"foo"), blob_hash(b"bar"));
    }

    #[test]
    fn output_is_lowercase_hex_40_chars() {
        let h = blob_hash(b"any content");
        assert_eq!(h.len(), 40);
        assert!(
            h.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
    }
}
