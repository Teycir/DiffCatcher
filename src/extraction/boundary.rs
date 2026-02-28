pub fn truncate_with_limit(code: &str, max_lines: u32) -> (String, bool, u32) {
    let lines: Vec<&str> = code.lines().collect();
    let actual = lines.len() as u32;
    if actual <= max_lines {
        return (code.to_string(), false, actual);
    }

    let clipped = lines
        .into_iter()
        .take(max_lines as usize)
        .collect::<Vec<_>>()
        .join("\n");
    (clipped, true, actual)
}
