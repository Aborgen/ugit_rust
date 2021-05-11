pub fn is_hex(s: &str) -> bool {
  s.chars().all(|c| match c {
    // 0-9
    '\u{0030}'..='\u{0039}' => true,
    // a-f
    '\u{0061}'..='\u{0066}' => true,
    _ => false
  })
}
