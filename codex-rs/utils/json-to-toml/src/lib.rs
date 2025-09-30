use serde_json::Value as JsonValue;
use toml::Value as TomlValue;

/// Convert a `serde_json::Value` into a semantically equivalent `toml::Value`.
pub fn json_to_toml(v: JsonValue) -> TomlValue {
    match v {
        JsonValue::Null => TomlValue::String(String::new()),
        JsonValue::Bool(b) => TomlValue::Boolean(b),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                TomlValue::Integer(i)
            } else if let Some(f) = n.as_f64() {
                TomlValue::Float(f)
            } else {
                TomlValue::String(n.to_string())
            }
        }
        JsonValue::String(s) => TomlValue::String(s),
        JsonValue::Array(arr) => TomlValue::Array(arr.into_iter().map(json_to_toml).collect()),
        JsonValue::Object(map) => {
            let tbl = map
                .into_iter()
                .map(|(k, v)| (k, json_to_toml(v)))
                .collect::<toml::value::Table>();
            TomlValue::Table(tbl)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn json_number_to_toml() {
        let json_value = json!(123);
        assert_eq!(TomlValue::Integer(123), json_to_toml(json_value));
    }

    #[test]
    fn json_array_to_toml() {
        let json_value = json!([true, 1]);
        assert_eq!(
            TomlValue::Array(vec![TomlValue::Boolean(true), TomlValue::Integer(1)]),
            json_to_toml(json_value)
        );
    }

    #[test]
    fn json_bool_to_toml() {
        let json_value = json!(false);
        assert_eq!(TomlValue::Boolean(false), json_to_toml(json_value));
    }

    #[test]
    fn json_float_to_toml() {
        let json_value = json!(1.25);
        assert_eq!(TomlValue::Float(1.25), json_to_toml(json_value));
    }

    #[test]
    fn json_null_to_toml() {
        let json_value = serde_json::Value::Null;
        assert_eq!(TomlValue::String(String::new()), json_to_toml(json_value));
    }

    #[test]
    fn json_object_nested() {
        let json_value = json!({ "outer": { "inner": 2 } });
        let expected = {
            let mut inner = toml::value::Table::new();
            inner.insert("inner".into(), TomlValue::Integer(2));

            let mut outer = toml::value::Table::new();
            outer.insert("outer".into(), TomlValue::Table(inner));
            TomlValue::Table(outer)
        };

        assert_eq!(json_to_toml(json_value), expected);
    }
}
