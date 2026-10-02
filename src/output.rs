//! Printing results: the API's JSON as it came (`--output json`), or tables.
//! Results go to stdout; notices and progress to stderr, so a pipe gets data only.

use comfy_table::presets::UTF8_BORDERS_ONLY;
use comfy_table::{ContentArrangement, Table};
use serde_json::Value;

use crate::cli::{Locale, OutputFormat};

/// A column of a table: its header, and the dotted path to its value.
pub struct Column {
    pub header: &'static str,
    pub path: &'static str,
}

pub const fn col(header: &'static str, path: &'static str) -> Column {
    Column { header, path }
}

#[derive(Debug, Clone, Copy)]
pub struct Output {
    pub format: OutputFormat,
    pub locale: Locale,
}

impl Output {
    pub fn is_json(self) -> bool {
        self.format == OutputFormat::Json
    }

    /// A list, one row per item.
    pub fn list(self, items: &Value, columns: &[Column]) {
        if self.is_json() {
            return json(items);
        }
        let rows = items.as_array().map(Vec::as_slice).unwrap_or_default();
        if rows.is_empty() {
            eprintln!("Nothing to show.");
            return;
        }
        let mut table = table();
        table.set_header(columns.iter().map(|c| c.header));
        for row in rows {
            table.add_row(columns.iter().map(|c| self.cell(lookup(row, c.path))));
        }
        println!("{table}");
    }

    /// One item, one row per field.
    pub fn item(self, item: &Value, fields: &[Column]) {
        if self.is_json() {
            return json(item);
        }
        let mut table = table();
        for field in fields {
            table.add_row([field.header.to_owned(), self.cell(lookup(item, field.path))]);
        }
        println!("{table}");
    }

    /// A document printed whole whatever the format (DID documents, schemas).
    pub fn document(self, value: &Value) {
        json(value);
    }

    /// A value as a table cell: `-` for nothing, texts by language in this
    /// locale, lists joined, objects as compact JSON.
    pub fn cell(self, value: &Value) -> String {
        match value {
            Value::Null => "-".to_owned(),
            Value::Bool(true) => "yes".to_owned(),
            Value::Bool(false) => "no".to_owned(),
            Value::String(text) => text.clone(),
            Value::Number(number) => number.to_string(),
            Value::Array(items) if items.is_empty() => "-".to_owned(),
            Value::Array(items) => items
                .iter()
                .map(|item| self.entry(item))
                .collect::<Vec<_>>()
                .join(", "),
            Value::Object(_) => self.text(value).unwrap_or_else(|| value.to_string()),
        }
    }

    /// An item of a list in a cell: an object by what names it.
    fn entry(self, item: &Value) -> String {
        ["name", "ref", "type", "id"]
            .iter()
            .find_map(|key| item.get(*key).filter(|v| !v.is_null()))
            .map_or_else(|| self.cell(item), |named| self.cell(named))
    }

    /// `{"en": …, "es": …}` in this locale, else English, else any.
    pub fn text(self, value: &Value) -> Option<String> {
        let texts = value.as_object()?;
        let by_language = !texts.is_empty()
            && texts.iter().all(|(key, text)| {
                key.len() == 2 && key.chars().all(|c| c.is_ascii_lowercase()) && text.is_string()
            });
        if !by_language {
            return None;
        }
        [self.locale.code(), "en"]
            .iter()
            .find_map(|language| texts.get(*language))
            .or_else(|| texts.values().next())
            .and_then(Value::as_str)
            .map(str::to_owned)
    }
}

pub fn json(value: &Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_default()
    );
}

/// A notice for the person at the terminal, never for a pipe.
pub fn notice(message: impl std::fmt::Display) {
    eprintln!("{message}");
}

fn table() -> Table {
    let mut table = Table::new();
    table
        .load_style(UTF8_BORDERS_ONLY)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table
}

/// The value at a dotted path (`mediator.name`), or `null`.
pub fn lookup<'a>(value: &'a Value, path: &str) -> &'a Value {
    path.split('.')
        .try_fold(value, |value, key| value.get(key))
        .unwrap_or(&Value::Null)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn output(locale: Locale) -> Output {
        Output {
            format: OutputFormat::Table,
            locale,
        }
    }

    #[test]
    fn cells_read_like_text() {
        let out = output(Locale::En);
        assert_eq!(out.cell(&Value::Null), "-");
        assert_eq!(out.cell(&json!(true)), "yes");
        assert_eq!(out.cell(&json!(["a", "b"])), "a, b");
        assert_eq!(out.cell(&json!({"a": 1})), r#"{"a":1}"#);
        assert_eq!(
            out.cell(&json!([{"kind": "issuer", "name": "Acme"}, {"ref": "email"}])),
            "Acme, email"
        );
    }

    #[test]
    fn texts_by_language_follow_the_locale() {
        let texts = json!({"en": "Name", "es": "Nombre"});
        assert_eq!(output(Locale::Es).cell(&texts), "Nombre");
        assert_eq!(output(Locale::En).cell(&texts), "Name");
        assert_eq!(output(Locale::Es).cell(&json!({"en": "Only"})), "Only");
    }

    #[test]
    fn paths_reach_into_objects() {
        let item = json!({"mediator": {"name": "Relay"}});
        assert_eq!(lookup(&item, "mediator.name"), &json!("Relay"));
        assert_eq!(lookup(&item, "mediator.url"), &Value::Null);
        assert_eq!(lookup(&item, "nothing.here"), &Value::Null);
    }
}
