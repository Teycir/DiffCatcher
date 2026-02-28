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

pub fn try_capture_full_element(code: &str) -> Option<String> {
    let lines = code.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return None;
    }

    if let Some(captured) = capture_brace_block(&lines) {
        return Some(captured);
    }
    capture_indentation_block(&lines)
}

fn capture_brace_block(lines: &[&str]) -> Option<String> {
    let start = lines
        .iter()
        .position(|line| line.contains('{') || line.trim_start().starts_with("fn "))?;

    let mut started = false;
    let mut depth = 0_i32;
    let mut end = None;

    for (offset, line) in lines.iter().enumerate().skip(start) {
        for ch in line.chars() {
            if ch == '{' {
                depth += 1;
                started = true;
            } else if ch == '}' && started {
                depth -= 1;
            }
        }

        if started && depth == 0 {
            end = Some(offset);
            break;
        }
    }

    end.map(|end_idx| lines[start..=end_idx].join("\n"))
}

fn capture_indentation_block(lines: &[&str]) -> Option<String> {
    let start = lines
        .iter()
        .position(|line| line.trim_end().ends_with(':') && !line.trim().is_empty())?;
    let base_indent = indentation(lines[start]);
    let mut end = start;

    for (idx, line) in lines.iter().enumerate().skip(start + 1) {
        if line.trim().is_empty() {
            end = idx;
            continue;
        }
        let indent = indentation(line);
        if indent <= base_indent {
            break;
        }
        end = idx;
    }

    if end > start {
        Some(lines[start..=end].join("\n"))
    } else {
        None
    }
}

fn indentation(line: &str) -> usize {
    line.chars().take_while(|c| c.is_whitespace()).count()
}
