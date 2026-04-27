const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

pub fn is_binary(content: &[u8]) -> bool {
    let probe_len = content.len().min(8000);
    content[..probe_len].iter().any(|&b| b == 0)
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
