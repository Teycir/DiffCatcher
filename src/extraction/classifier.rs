use std::path::Path;

use crate::types::Language;

pub fn classify_language(path: &str) -> Language {
    let p = Path::new(path);

    if p.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("Dockerfile"))
    {
        return Language::Dockerfile;
    }

    match p.extension().and_then(|e| e.to_str()).unwrap_or_default() {
        "rs" => Language::Rust,
        "py" => Language::Python,
        "js" | "jsx" | "mjs" | "cjs" => Language::JavaScript,
        "ts" | "tsx" => Language::TypeScript,
        "go" => Language::Go,
        "c" | "h" => Language::C,
        "cc" | "cpp" | "cxx" | "hpp" | "hh" => Language::Cpp,
        "java" => Language::Java,
        "kt" | "kts" => Language::Kotlin,
        "rb" => Language::Ruby,
        "toml" => Language::Toml,
        "yaml" | "yml" => Language::Yaml,
        "json" => Language::Json,
        "md" => Language::Markdown,
        "sh" | "bash" | "zsh" => Language::Shell,
        other => Language::Unknown(other.to_string()),
    }
}
