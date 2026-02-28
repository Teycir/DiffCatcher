use crate::error::Result;

pub fn to_pretty_json<T: serde::Serialize>(value: &T) -> Result<String> {
    Ok(serde_json::to_string_pretty(value)?)
}
