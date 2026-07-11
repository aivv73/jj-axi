pub enum ToonValue {
    Null,
    Bool(bool),
    UInt(u64),
    String(String),
    Object(Vec<(&'static str, ToonValue)>),
    Array(Vec<ToonValue>),
}

pub fn render(value: &ToonValue) -> String {
    let mut output = String::new();

    match value {
        ToonValue::Object(fields) => write_object(fields, 0, &mut output),
        ToonValue::Array(values) => write_root_array(values, &mut output),
        _ => write_primitive(value, &mut output),
    }

    output
}

fn write_object(fields: &[(&'static str, ToonValue)], indent: usize, output: &mut String) {
    for (key, value) in fields {
        write_field(key, value, indent, output);
    }
}

fn write_field(key: &str, value: &ToonValue, indent: usize, output: &mut String) {
    if is_primitive(value) {
        begin_line(output, indent);
        write_key(key, output);
        output.push_str(": ");
        write_primitive(value, output);
        return;
    }

    match value {
        ToonValue::Object(fields) => {
            begin_line(output, indent);
            write_key(key, output);
            output.push(':');
            write_object(fields, indent + 1, output);
        }
        ToonValue::Array(values) => write_field_array(key, values, indent, output),
        _ => unreachable!("primitive values are handled before this match"),
    }
}

fn write_field_array(key: &str, values: &[ToonValue], indent: usize, output: &mut String) {
    begin_line(output, indent);
    write_key(key, output);

    if values.is_empty() {
        output.push_str(": []");
        return;
    }

    write_array_header(values.len(), output);
    if values_are_primitive(values) {
        output.push(' ');
        write_inline_values(values, output);
    } else {
        for value in values {
            write_list_item(value, indent + 1, output);
        }
    }
}

fn write_root_array(values: &[ToonValue], output: &mut String) {
    if values.is_empty() {
        output.push_str("[]");
        return;
    }

    write_array_header(values.len(), output);
    if values_are_primitive(values) {
        output.push(' ');
        write_inline_values(values, output);
    } else {
        for value in values {
            write_list_item(value, 1, output);
        }
    }
}

fn write_list_item(value: &ToonValue, indent: usize, output: &mut String) {
    if is_primitive(value) {
        begin_line(output, indent);
        output.push_str("- ");
        write_primitive(value, output);
        return;
    }

    match value {
        ToonValue::Object(fields) => write_object_list_item(fields, indent, output),
        ToonValue::Array(values) => write_array_list_item(values, indent, output),
        _ => unreachable!("primitive values are handled before this match"),
    }
}

fn write_object_list_item(
    fields: &[(&'static str, ToonValue)],
    indent: usize,
    output: &mut String,
) {
    let Some(((first_key, first_value), remaining_fields)) = fields.split_first() else {
        begin_line(output, indent);
        output.push('-');
        return;
    };

    write_first_object_field(first_key, first_value, indent, output);
    write_object(remaining_fields, indent + 1, output);
}

fn write_first_object_field(key: &str, value: &ToonValue, indent: usize, output: &mut String) {
    begin_line(output, indent);
    output.push_str("- ");
    write_key(key, output);

    if is_primitive(value) {
        output.push_str(": ");
        write_primitive(value, output);
        return;
    }

    match value {
        ToonValue::Object(fields) => {
            output.push(':');
            write_object(fields, indent + 1, output);
        }
        ToonValue::Array(values) => {
            if values.is_empty() {
                output.push_str(": []");
                return;
            }

            write_array_header(values.len(), output);
            if values_are_primitive(values) {
                output.push(' ');
                write_inline_values(values, output);
            } else {
                for value in values {
                    write_list_item(value, indent + 1, output);
                }
            }
        }
        _ => unreachable!("primitive values are handled before this match"),
    }
}

fn write_array_list_item(values: &[ToonValue], indent: usize, output: &mut String) {
    begin_line(output, indent);
    output.push_str("- ");
    write_array_header(values.len(), output);

    if values.is_empty() {
        return;
    }

    if values_are_primitive(values) {
        output.push(' ');
        write_inline_values(values, output);
    } else {
        for value in values {
            write_list_item(value, indent + 1, output);
        }
    }
}

fn write_array_header(length: usize, output: &mut String) {
    output.push('[');
    push_usize(length, output);
    output.push_str("]:");
}

fn write_inline_values(values: &[ToonValue], output: &mut String) {
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        write_primitive(value, output);
    }
}

fn write_primitive(value: &ToonValue, output: &mut String) {
    match value {
        ToonValue::Null => output.push_str("null"),
        ToonValue::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        ToonValue::UInt(value) => push_u64(*value, output),
        ToonValue::String(value) => write_string(value, output),
        ToonValue::Object(_) | ToonValue::Array(_) => {
            unreachable!("only primitive values can be written inline")
        }
    }
}

fn write_string(value: &str, output: &mut String) {
    if requires_quotes(value) {
        write_quoted(value, output);
    } else {
        output.push_str(value);
    }
}

fn write_key(key: &str, output: &mut String) {
    if is_bare_key(key) {
        output.push_str(key);
    } else {
        write_quoted(key, output);
    }
}

fn write_quoted(value: &str, output: &mut String) {
    output.push('"');

    for character in value.chars() {
        match character {
            '\\' => output.push_str("\\\\"),
            '"' => output.push_str("\\\""),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            '\u{0000}'..='\u{001F}' => {
                output.push_str("\\u");
                push_lowercase_hex_u16(character as u32, output);
            }
            _ => output.push(character),
        }
    }

    output.push('"');
}

fn requires_quotes(value: &str) -> bool {
    value.is_empty()
        || value.chars().next().is_some_and(char::is_whitespace)
        || value.chars().next_back().is_some_and(char::is_whitespace)
        || matches!(value, "true" | "false" | "null")
        || is_numeric_like(value)
        || value.starts_with('-')
        || value.chars().any(|character| {
            matches!(character, ':' | '"' | '\\' | '[' | ']' | '{' | '}' | ',')
                || ('\u{0000}'..='\u{001F}').contains(&character)
        })
}

fn is_numeric_like(value: &str) -> bool {
    let bytes = value.as_bytes();
    let mut index = 0;

    if bytes.first() == Some(&b'-') {
        index += 1;
    }

    let integer_start = index;
    while bytes.get(index).is_some_and(u8::is_ascii_digit) {
        index += 1;
    }
    if index == integer_start {
        return false;
    }

    if bytes.get(index) == Some(&b'.') {
        index += 1;
        let fraction_start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if index == fraction_start {
            return false;
        }
    }

    if matches!(bytes.get(index), Some(b'e' | b'E')) {
        index += 1;
        if matches!(bytes.get(index), Some(b'+' | b'-')) {
            index += 1;
        }

        let exponent_start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if index == exponent_start {
            return false;
        }
    }

    index == bytes.len()
}

fn is_bare_key(key: &str) -> bool {
    let mut bytes = key.bytes();

    match bytes.next() {
        Some(b'a'..=b'z' | b'A'..=b'Z' | b'_') => {}
        _ => return false,
    }

    bytes.all(|byte| matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'.'))
}

fn is_primitive(value: &ToonValue) -> bool {
    matches!(
        value,
        ToonValue::Null | ToonValue::Bool(_) | ToonValue::UInt(_) | ToonValue::String(_)
    )
}

fn values_are_primitive(values: &[ToonValue]) -> bool {
    values.iter().all(is_primitive)
}

fn begin_line(output: &mut String, indent: usize) {
    if !output.is_empty() {
        output.push('\n');
    }

    for _ in 0..indent {
        output.push_str("  ");
    }
}

fn push_lowercase_hex_u16(value: u32, output: &mut String) {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    for shift in [12, 8, 4, 0] {
        output.push(HEX[((value >> shift) & 0xF) as usize] as char);
    }
}

fn push_u64(value: u64, output: &mut String) {
    let mut digits = [0_u8; 20];
    let mut value = value;
    let mut first = digits.len();

    loop {
        first -= 1;
        digits[first] = b'0' + (value % 10) as u8;
        value /= 10;

        if value == 0 {
            break;
        }
    }

    for digit in &digits[first..] {
        output.push(*digit as char);
    }
}

fn push_usize(value: usize, output: &mut String) {
    let mut digits = [0_u8; 20];
    let mut value = value;
    let mut first = digits.len();

    loop {
        first -= 1;
        digits[first] = b'0' + (value % 10) as u8;
        value /= 10;

        if value == 0 {
            break;
        }
    }

    for digit in &digits[first..] {
        output.push(*digit as char);
    }
}

#[cfg(test)]
mod tests {
    use super::{ToonValue, render};

    #[test]
    fn renders_nested_values_with_canonical_escaping() {
        let value = ToonValue::Object(vec![
            ("literal", ToonValue::String("true".to_owned())),
            ("control", ToonValue::String("a\u{0001}\n".to_owned())),
            (
                "items",
                ToonValue::Array(vec![
                    ToonValue::String("one".to_owned()),
                    ToonValue::String("two,three".to_owned()),
                ]),
            ),
            (
                "records",
                ToonValue::Array(vec![ToonValue::Object(vec![
                    (
                        "state",
                        ToonValue::Object(vec![("conflicted", ToonValue::Bool(false))]),
                    ),
                    ("name", ToonValue::String("example".to_owned())),
                ])]),
            ),
        ]);

        assert_eq!(
            render(&value),
            concat!(
                "literal: \"true\"\n",
                "control: \"a\\u0001\\n\"\n",
                "items[2]: one,\"two,three\"\n",
                "records[1]:\n",
                "  - state:\n",
                "    conflicted: false\n",
                "    name: example"
            )
        );
    }
}
