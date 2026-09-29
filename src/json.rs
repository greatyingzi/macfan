//! A minimal JSON writer.
//!
//! `rsmc --json` needs structured output and this crate has no dependencies, so
//! this is a small builder rather than a serialisation framework: objects and
//! arrays of strings, with the number formatting decisions made explicitly
//! (integers stay integers, which matters for anything parsing the output).

/// Escape a string for use inside JSON quotes.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// A quoted JSON string.
pub fn string(s: &str) -> String {
    format!("\"{}\"", escape(s))
}

/// A JSON number: whole values are written without a decimal point.
pub fn number(v: f64) -> String {
    if !v.is_finite() {
        return "null".to_string();
    }
    if v.fract() == 0.0 && v.abs() < 9.0e15 {
        format!("{}", v as i64)
    } else {
        // Round to a sane precision: these are sensor readings, not money.
        let rounded = (v * 1000.0).round() / 1000.0;
        format!("{rounded}")
    }
}

/// An object under construction; field order is preserved.
#[derive(Default)]
pub struct Obj {
    fields: Vec<(String, String)>,
}

impl Obj {
    /// An empty object.
    pub fn new() -> Self {
        Obj::default()
    }

    /// Add a string field.
    pub fn str_field(&mut self, key: &str, value: &str) -> &mut Self {
        self.fields.push((string(key), string(value)));
        self
    }

    /// Add a field that is already valid JSON (number, object, array, null).
    pub fn raw_field(&mut self, key: &str, value: String) -> &mut Self {
        self.fields.push((string(key), value));
        self
    }

    /// Add a boolean field.
    pub fn bool_field(&mut self, key: &str, value: bool) -> &mut Self {
        self.raw_field(key, if value { "true".into() } else { "false".into() })
    }

    /// Add an array of ready-made values.
    pub fn array_field(&mut self, key: &str, items: &[String]) -> &mut Self {
        self.raw_field(key, array(items))
    }

    /// Render the object.
    pub fn render(&self) -> String {
        let body: Vec<String> = self
            .fields
            .iter()
            .map(|(key, value)| format!("{key}:{value}"))
            .collect();
        format!("{{{}}}", body.join(","))
    }
}

/// Render an array of ready-made values.
pub fn array(items: &[String]) -> String {
    format!("[{}]", items.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_are_escaped() {
        assert_eq!(string("plain"), "\"plain\"");
        assert_eq!(string("a\"b"), "\"a\\\"b\"");
        assert_eq!(string("back\\slash"), "\"back\\\\slash\"");
        assert_eq!(string("line\nbreak"), "\"line\\nbreak\"");
        assert_eq!(string("tab\t"), "\"tab\\t\"");
        // Control characters become escapes; non-ASCII passes through as UTF-8.
        assert_eq!(string("\u{1}"), "\"\\u0001\"");
        assert_eq!(string("风扇"), "\"风扇\"");
    }

    #[test]
    fn numbers_stay_integers_when_whole() {
        assert_eq!(number(7199.0), "7199");
        // Out of i64 range: must not saturate to i64::MAX.
        assert_eq!(number(1.0e19), "10000000000000000000");
        assert_eq!(number(0.0), "0");
        assert_eq!(number(-206.0), "-206");
        assert_eq!(number(30.5), "30.5");
        assert_eq!(number(30.500_001), "30.5");
        assert_eq!(number(f64::NAN), "null");
        assert_eq!(number(f64::INFINITY), "null");
    }

    #[test]
    fn objects_and_arrays_render() {
        let mut obj = Obj::new();
        obj.raw_field("index", number(0.0))
            .str_field("type", "flt ")
            .bool_field("forced", true)
            .array_field("bytes", &[number(0.0), number(248.0)]);
        assert_eq!(
            obj.render(),
            "{\"index\":0,\"type\":\"flt \",\"forced\":true,\"bytes\":[0,248]}"
        );
        assert_eq!(array(&[]), "[]");
        assert_eq!(array(&[string("a"), string("b")]), "[\"a\",\"b\"]");
    }
}
