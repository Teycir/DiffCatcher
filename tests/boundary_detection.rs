use diffcatcher::extraction::boundary::try_capture_full_element;

#[test]
fn captures_kr_style_function_block() {
    let code = r#"fn validate_token(token: &str) -> bool {
    if token.is_empty() {
        return false;
    }
    true
}
let untouched = 1;
"#;

    let captured = try_capture_full_element(code).expect("expected K&R capture");
    assert!(captured.contains("fn validate_token"));
    assert!(captured.contains("if token.is_empty()"));
    assert!(!captured.contains("let untouched = 1;"));
}

#[test]
fn captures_allman_style_function_block() {
    let code = r#"fn authorize(user: &User) -> bool
{
    if user.is_admin() {
        return true;
    }
    false
}
let sentinel = 0;
"#;

    let captured = try_capture_full_element(code).expect("expected Allman capture");
    assert!(captured.contains("fn authorize"));
    assert!(captured.contains("if user.is_admin()"));
    assert!(!captured.contains("let sentinel = 0;"));
}

#[test]
fn captures_python_indentation_block() {
    let code = r#"def check_permissions(user, resource):
    if user.is_admin:
        return True
    return has_access(user, resource)

print("outside")
"#;

    let captured = try_capture_full_element(code).expect("expected indentation capture");
    assert!(captured.contains("def check_permissions"));
    assert!(captured.contains("return has_access"));
    assert!(!captured.contains("print(\"outside\")"));
}

#[test]
fn captures_single_line_function() {
    let code = "fn fast() -> bool { true }\nlet next = 1;\n";
    let captured = try_capture_full_element(code).expect("expected single-line capture");
    assert_eq!(captured.trim(), "fn fast() -> bool { true }");
}
