const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

pub fn is_binary(content: &[u8]) -> bool {
    let probe_len = content.len().min(8000);
    content[..probe_len].contains(&0)
}

pub fn normalize_text(content: &[u8], keep_bom: bool) -> Vec<u8> {
    let body = if !keep_bom && content.starts_with(UTF8_BOM) {
        &content[UTF8_BOM.len()..]
    } else {
        content
    };

    let mut out = Vec::with_capacity(body.len());
    let mut i = 0;
    while i < body.len() {
        if body[i] == b'\r' && body.get(i + 1) == Some(&b'\n') {
            out.push(b'\n');
            i += 2;
        } else {
            out.push(body[i]);
            i += 1;
        }
    }
    out
}

pub fn prepare_for_hash(content: &[u8], keep_bom: bool) -> (Vec<u8>, bool) {
    if is_binary(content) {
        (content.to_vec(), true)
    } else {
        (normalize_text(content, keep_bom), false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_bom_when_keep_bom_false() {
        let input = [0xEF, 0xBB, 0xBF, b'a'];
        assert_eq!(normalize_text(&input, false), b"a");
    }

    #[test]
    fn keeps_bom_when_keep_bom_true() {
        let input = [0xEF, 0xBB, 0xBF, b'a'];
        assert_eq!(normalize_text(&input, true), input);
    }

    #[test]
    fn crlf_normalized_to_lf() {
        assert_eq!(
            normalize_text(b"hello\r\nworld\r\n", false),
            b"hello\nworld\n"
        );
    }

    #[test]
    fn lone_cr_is_preserved() {
        assert_eq!(normalize_text(b"a\rb", false), b"a\rb");
    }

    #[test]
    fn detects_binary_with_nul_byte() {
        assert!(is_binary(&[0u8, 1, 2]));
        assert!(!is_binary(b"plain text"));
    }

    #[test]
    fn binary_probe_is_capped_at_first_8000_bytes() {
        let mut content = vec![b'a'; 9000];
        content.push(0);
        assert!(!is_binary(&content));
    }

    #[test]
    fn prepare_for_hash_returns_correct_flag() {
        let (text_out, text_flag) = prepare_for_hash(b"hello\r\n", false);
        assert_eq!(text_out, b"hello\n");
        assert!(!text_flag);

        let binary = [0u8, 1, 2, 3];
        let (bin_out, bin_flag) = prepare_for_hash(&binary, false);
        assert_eq!(bin_out, binary);
        assert!(bin_flag);
    }

    #[test]
    fn prepare_for_hash_keeps_bom_when_requested() {
        let input = [0xEF, 0xBB, 0xBF, b'a'];
        let (out, is_bin) = prepare_for_hash(&input, true);
        assert_eq!(out, input);
        assert!(!is_bin);
    }
}
