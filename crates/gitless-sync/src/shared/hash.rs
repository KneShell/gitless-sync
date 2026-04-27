use sha1::{Digest, Sha1};

pub fn blob_hash(content: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(format!("blob {}\0", content.len()).as_bytes());
    hasher.update(content);
    let result = hasher.finalize();
    hex_encode(&result)
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_blob_matches_git() {
        // git's empty blob SHA-1 is the well-known constant
        assert_eq!(blob_hash(&[]), "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391");
    }
}
