/// Shared pure parsing logic (same rules as the C version).
pub enum Num {
    Int(i64),
    Real(f64),
}

pub fn parse_number(s: &[u8]) -> Option<Num> {
    let s = s.trim_ascii();
    let mut buf: Vec<u8> = Vec::with_capacity(s.len());
    let (mut ndigits, mut seen_dot, mut neg) = (0usize, false, false);
    let mut i = 0;
    while i < s.len() {
        let c = s[i];
        match c {
            b',' | b'$' | b'%' => {}
            0xC2 if s.get(i + 1) == Some(&0xA3) => i += 1, // £ in UTF-8
            b'0'..=b'9' => {
                buf.push(c);
                ndigits += 1;
            }
            b'.' if !seen_dot => {
                seen_dot = true;
                buf.push(c);
            }
            b'-' if !neg && buf.is_empty() => {
                neg = true;
                buf.push(c);
            }
            _ => return None,
        }
        i += 1;
    }
    if ndigits == 0 {
        return None;
    }
    // Only ASCII digits, '-' and '.' are in buf, so this is valid UTF-8.
    let t = std::str::from_utf8(&buf).ok()?;
    if !seen_dot {
        if let Ok(n) = t.parse::<i64>() {
            return Some(Num::Int(n));
        }
    }
    t.parse::<f64>().ok().map(Num::Real)
}
