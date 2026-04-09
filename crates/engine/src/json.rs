use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(i64),
    String(String),
    Array(Vec<JsonValue>),
    Object(BTreeMap<String, JsonValue>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonError {
    message: String,
}

impl JsonError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for JsonError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for JsonError {}

impl JsonValue {
    #[must_use]
    pub fn render(&self) -> String {
        match self {
            Self::Null => "null".to_string(),
            Self::Bool(value) => value.to_string(),
            Self::Number(value) => value.to_string(),
            Self::String(value) => render_string(value),
            Self::Array(values) => {
                let rendered = values
                    .iter()
                    .map(Self::render)
                    .collect::<Vec<_>>()
                    .join(",");
                format!("[{rendered}]")
            }
            Self::Object(entries) => {
                let rendered = entries
                    .iter()
                    .map(|(key, value)| format!("{}:{}", render_string(key), value.render()))
                    .collect::<Vec<_>>()
                    .join(",");
                format!("{{{rendered}}}")
            }
        }
    }

    pub fn parse(source: &str) -> Result<Self, JsonError> {
        let mut parser = Parser::new(source);
        let value = parser.parse_value()?;
        parser.skip_whitespace();
        if parser.is_eof() {
            Ok(value)
        } else {
            Err(JsonError::new("unexpected trailing content"))
        }
    }

    #[must_use]
    pub fn as_object(&self) -> Option<&BTreeMap<String, JsonValue>> {
        match self {
            Self::Object(value) => Some(value),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_array(&self) -> Option<&[JsonValue]> {
        match self {
            Self::Array(value) => Some(value),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Number(value) => Some(*value),
            _ => None,
        }
    }
}

fn render_string(value: &str) -> String {
    let mut rendered = String::with_capacity(value.len() + 2);
    rendered.push('"');
    for ch in value.chars() {
        match ch {
            '"' => rendered.push_str("\\\""),
            '\\' => rendered.push_str("\\\\"),
            '\n' => rendered.push_str("\\n"),
            '\r' => rendered.push_str("\\r"),
            '\t' => rendered.push_str("\\t"),
            '\u{08}' => rendered.push_str("\\b"),
            '\u{0C}' => rendered.push_str("\\f"),
            control if control.is_control() => push_unicode_escape(&mut rendered, control),
            plain => rendered.push(plain),
        }
    }
    rendered.push('"');
    rendered
}

fn push_unicode_escape(rendered: &mut String, control: char) {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    rendered.push_str("\\u");
    let value = u32::from(control);
    for shift in [12_u32, 8, 4, 0] {
        let nibble = ((value >> shift) & 0xF) as usize;
        rendered.push(char::from(HEX[nibble]));
    }
}

struct Parser<'a> {
    chars: Vec<char>,
    index: usize,
    _source: &'a str,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            chars: source.chars().collect(),
            index: 0,
            _source: source,
        }
    }

    fn parse_value(&mut self) -> Result<JsonValue, JsonError> {
        self.skip_whitespace();
        match self.peek() {
            Some('n') => self.parse_literal("null", JsonValue::Null),
            Some('t') => self.parse_literal("true", JsonValue::Bool(true)),
            Some('f') => self.parse_literal("false", JsonValue::Bool(false)),
            Some('"') => self.parse_string().map(JsonValue::String),
            Some('[') => self.parse_array(),
            Some('{') => self.parse_object(),
            Some('-' | '0'..='9') => self.parse_number().map(JsonValue::Number),
            Some(other) => Err(JsonError::new(format!("unexpected character: {other}"))),
            None => Err(JsonError::new("unexpected end of input")),
        }
    }

    fn parse_literal(&mut self, expected: &str, value: JsonValue) -> Result<JsonValue, JsonError> {
        for expected_char in expected.chars() {
            if self.next() != Some(expected_char) {
                return Err(JsonError::new(format!(
                    "invalid literal: expected {expected}"
                )));
            }
        }
        Ok(value)
    }

    fn parse_string(&mut self) -> Result<String, JsonError> {
        self.expect('"')?;
        let mut value = String::new();
        while let Some(ch) = self.next() {
            match ch {
                '"' => return Ok(value),
                '\\' => value.push(self.parse_escape()?),
                plain => value.push(plain),
            }
        }
        Err(JsonError::new("unterminated string"))
    }

    fn parse_escape(&mut self) -> Result<char, JsonError> {
        match self.next() {
            Some('"') => Ok('"'),
            Some('\\') => Ok('\\'),
            Some('/') => Ok('/'),
            Some('b') => Ok('\u{08}'),
            Some('f') => Ok('\u{0C}'),
            Some('n') => Ok('\n'),
            Some('r') => Ok('\r'),
            Some('t') => Ok('\t'),
            Some('u') => self.parse_unicode_escape(),
            Some(other) => Err(JsonError::new(format!("invalid escape sequence: {other}"))),
            None => Err(JsonError::new("unexpected end of input in escape sequence")),
        }
    }

    fn parse_unicode_escape(&mut self) -> Result<char, JsonError> {
        let mut value = 0_u32;
        for _ in 0..4 {
            let Some(ch) = self.next() else {
                return Err(JsonError::new("unexpected end of input in unicode escape"));
            };
            value = (value << 4)
                | ch.to_digit(16)
                    .ok_or_else(|| JsonError::new("invalid unicode escape"))?;
        }
        char::from_u32(value).ok_or_else(|| JsonError::new("invalid unicode scalar value"))
    }

    fn parse_array(&mut self) -> Result<JsonValue, JsonError> {
        self.expect('[')?;
        let mut values = Vec::new();
        loop {
            self.skip_whitespace();
            if self.try_consume(']') {
                break;
            }
            values.push(self.parse_value()?);
            self.skip_whitespace();
            if self.try_consume(']') {
                break;
            }
            self.expect(',')?;
        }
        Ok(JsonValue::Array(values))
    }

    fn parse_object(&mut self) -> Result<JsonValue, JsonError> {
        self.expect('{')?;
        let mut entries = BTreeMap::new();
        loop {
            self.skip_whitespace();
            if self.try_consume('}') {
                break;
            }
            let key = self.parse_string()?;
            self.skip_whitespace();
            self.expect(':')?;
            let value = self.parse_value()?;
            entries.insert(key, value);
            self.skip_whitespace();
            if self.try_consume('}') {
                break;
            }
            self.expect(',')?;
        }
        Ok(JsonValue::Object(entries))
    }

    fn parse_number(&mut self) -> Result<i64, JsonError> {
        let mut value = String::new();
        if self.try_consume('-') {
            value.push('-');
        }

        while let Some(ch @ '0'..='9') = self.peek() {
            value.push(ch);
            self.index += 1;
        }

        if value.is_empty() || value == "-" {
            return Err(JsonError::new("invalid number"));
        }

        value
            .parse::<i64>()
            .map_err(|_| JsonError::new("number out of range"))
    }

    fn expect(&mut self, expected: char) -> Result<(), JsonError> {
        match self.next() {
            Some(actual) if actual == expected => Ok(()),
            Some(actual) => Err(JsonError::new(format!(
                "expected '{expected}', found '{actual}'"
            ))),
            None => Err(JsonError::new(format!(
                "expected '{expected}', found end of input"
            ))),
        }
    }

    fn try_consume(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(' ' | '\n' | '\r' | '\t')) {
            self.index += 1;
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.index).copied()
    }

    fn next(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.index += 1;
        Some(ch)
    }

    fn is_eof(&self) -> bool {
        self.index >= self.chars.len()
    }
}

#[cfg(test)]
mod tests {
    use super::{render_string, JsonValue};
    use std::collections::BTreeMap;

    #[test]
    fn renders_and_parses_json_values() {
        let mut object = BTreeMap::new();
        object.insert("flag".to_string(), JsonValue::Bool(true));
        object.insert(
            "items".to_string(),
            JsonValue::Array(vec![
                JsonValue::Number(4),
                JsonValue::String("ok".to_string()),
            ]),
        );

        let rendered = JsonValue::Object(object).render();
        let parsed = JsonValue::parse(&rendered).expect("json should parse");

        assert_eq!(parsed.as_object().expect("object").len(), 2);
    }

    #[test]
    fn escapes_control_characters() {
        assert_eq!(render_string("a\n\t\"b"), "\"a\\n\\t\\\"b\"");
    }

    #[test]
    fn renders_all_escape_sequences() {
        assert_eq!(render_string("\r"), "\"\\r\"");
        assert_eq!(render_string("\u{08}"), "\"\\b\"");
        assert_eq!(render_string("\u{0C}"), "\"\\f\"");
        assert_eq!(render_string("\\"), "\"\\\\\"");
        assert!(render_string("\u{01}").contains("\\u0001"));
    }

    #[test]
    fn renders_primitive_types() {
        assert_eq!(JsonValue::Null.render(), "null");
        assert_eq!(JsonValue::Bool(true).render(), "true");
        assert_eq!(JsonValue::Bool(false).render(), "false");
        assert_eq!(JsonValue::Number(42).render(), "42");
        assert_eq!(JsonValue::Number(-7).render(), "-7");
        assert_eq!(JsonValue::String("hi".into()).render(), "\"hi\"");
        assert_eq!(JsonValue::Array(vec![]).render(), "[]");
    }

    #[test]
    fn as_accessors_return_none_for_wrong_types() {
        let null = JsonValue::Null;
        assert!(null.as_object().is_none());
        assert!(null.as_array().is_none());
        assert!(null.as_str().is_none());
        assert!(null.as_bool().is_none());
        assert!(null.as_i64().is_none());

        let num = JsonValue::Number(5);
        assert!(num.as_str().is_none());
        assert!(num.as_bool().is_none());
        assert!(num.as_object().is_none());
        assert!(num.as_array().is_none());
        assert_eq!(num.as_i64(), Some(5));

        let b = JsonValue::Bool(true);
        assert_eq!(b.as_bool(), Some(true));
        assert!(b.as_i64().is_none());
    }

    #[test]
    fn parses_null_true_false_literals() {
        assert_eq!(JsonValue::parse("null").unwrap(), JsonValue::Null);
        assert_eq!(JsonValue::parse("true").unwrap(), JsonValue::Bool(true));
        assert_eq!(JsonValue::parse("false").unwrap(), JsonValue::Bool(false));
    }

    #[test]
    fn parses_numbers_including_negative() {
        assert_eq!(JsonValue::parse("0").unwrap(), JsonValue::Number(0));
        assert_eq!(JsonValue::parse("-42").unwrap(), JsonValue::Number(-42));
        assert_eq!(JsonValue::parse("12345").unwrap(), JsonValue::Number(12345));
    }

    #[test]
    fn parses_string_escapes() {
        let parsed = JsonValue::parse(r#""a\nb\t\\\/\"\u0041""#).unwrap();
        assert_eq!(parsed.as_str().unwrap(), "a\nb\t\\/\"A");
    }

    #[test]
    fn parses_arrays_and_nested_objects() {
        let parsed = JsonValue::parse(r#"[1, "two", [3], {"k": null}]"#).unwrap();
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 4);
        assert_eq!(arr[0].as_i64(), Some(1));
        assert_eq!(arr[2].as_array().unwrap().len(), 1);
    }

    #[test]
    fn rejects_trailing_content() {
        let err = JsonValue::parse("null extra").unwrap_err();
        assert!(err.to_string().contains("trailing"));
    }

    #[test]
    fn rejects_invalid_literal() {
        assert!(JsonValue::parse("nul").is_err());
        assert!(JsonValue::parse("tru").is_err());
        assert!(JsonValue::parse("fals").is_err());
    }

    #[test]
    fn rejects_unexpected_character() {
        let err = JsonValue::parse("@").unwrap_err();
        assert!(err.to_string().contains("unexpected character"));
    }

    #[test]
    fn rejects_empty_input() {
        let err = JsonValue::parse("").unwrap_err();
        assert!(err.to_string().contains("end of input"));
    }

    #[test]
    fn rejects_unterminated_string() {
        let err = JsonValue::parse(r#""no end"#).unwrap_err();
        assert!(err.to_string().contains("unterminated"));
    }

    #[test]
    fn rejects_invalid_escape() {
        let err = JsonValue::parse(r#""\x""#).unwrap_err();
        assert!(err.to_string().contains("invalid escape"));
    }

    #[test]
    fn rejects_truncated_unicode_escape() {
        let err = JsonValue::parse(r#""\u00""#).unwrap_err();
        assert!(err.to_string().contains("unicode escape"));
    }

    #[test]
    fn rejects_bad_unicode_hex() {
        let err = JsonValue::parse(r#""\u00GG""#).unwrap_err();
        assert!(err.to_string().contains("unicode"));
    }

    #[test]
    fn rejects_bare_minus() {
        let err = JsonValue::parse("-").unwrap_err();
        assert!(err.to_string().contains("invalid number"));
    }

    #[test]
    fn rejects_number_overflow() {
        let err = JsonValue::parse("99999999999999999999").unwrap_err();
        assert!(err.to_string().contains("out of range"));
    }

    #[test]
    fn rejects_bad_array_separator() {
        let err = JsonValue::parse("[1 2]").unwrap_err();
        assert!(err.to_string().contains("expected"));
    }

    #[test]
    fn rejects_bad_object_separator() {
        let err = JsonValue::parse(r#"{"a":1 "b":2}"#).unwrap_err();
        assert!(err.to_string().contains("expected"));
    }

    #[test]
    fn json_error_display() {
        let err = super::JsonError::new("test error");
        assert_eq!(err.to_string(), "test error");
    }
}
