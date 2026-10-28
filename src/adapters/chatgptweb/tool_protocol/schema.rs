use super::{Result, Value};

// Reject unknown validation keywords instead of silently weakening a declaration.
pub(super) fn check_schema(schema: &Value, depth: usize) -> Result<()> {
    if depth > 32 {
        return Err("schema nesting limit");
    }
    let object = schema.as_object().ok_or("expected schema object")?;
    for (key, value) in object {
        match key.as_str() {
            "type" => {
                let valid = |v: &Value| {
                    matches!(
                        v.as_str(),
                        Some(
                            "object"
                                | "array"
                                | "string"
                                | "number"
                                | "integer"
                                | "boolean"
                                | "null"
                        )
                    )
                };
                if !valid(value)
                    && !value
                        .as_array()
                        .is_some_and(|a| !a.is_empty() && a.iter().all(valid))
                {
                    return Err("unsupported schema type");
                }
            }
            "properties" => {
                for child in value.as_object().ok_or("invalid properties")?.values() {
                    check_schema(child, depth + 1)?;
                }
            }
            "items" => check_schema(value, depth + 1)?,
            "required" => {
                if !value
                    .as_array()
                    .is_some_and(|a| a.iter().all(Value::is_string))
                {
                    return Err("invalid required");
                }
            }
            "additionalProperties" => {
                if !value.is_boolean() {
                    check_schema(value, depth + 1)?;
                }
            }
            "anyOf" | "oneOf" | "allOf" => {
                let children = value
                    .as_array()
                    .filter(|a| !a.is_empty())
                    .ok_or("invalid schema alternatives")?;
                for child in children {
                    check_schema(child, depth + 1)?;
                }
            }
            "minItems" => {
                if value.as_u64().is_none() {
                    return Err("invalid minItems");
                }
            }
            "encrypted" => {
                if !value.is_boolean() {
                    return Err("invalid encrypted annotation");
                }
            }
            "enum" => {
                if !value.as_array().is_some_and(|a| !a.is_empty()) {
                    return Err("invalid enum");
                }
            }
            "description" | "title" | "default" | "$schema" | "examples" | "deprecated" => {}
            _ => return Err("unsupported schema keyword"),
        }
    }
    Ok(())
}
pub(super) fn validate(schema: &Value, value: &Value) -> Result<()> {
    if let Some(types) = schema.get("type") {
        let matches = |kind: &Value| match kind.as_str().unwrap_or("") {
            "object" => value.is_object(),
            "array" => value.is_array(),
            "string" => value.is_string(),
            "number" => value.is_number(),
            "integer" => value.is_i64() || value.is_u64(),
            "boolean" => value.is_boolean(),
            "null" => value.is_null(),
            _ => false,
        };
        if !matches(types) && !types.as_array().is_some_and(|a| a.iter().any(matches)) {
            return Err("argument type mismatch");
        }
    }
    if schema["enum"]
        .as_array()
        .is_some_and(|a| !a.contains(value))
    {
        return Err("argument enum mismatch");
    }
    for keyword in ["anyOf", "oneOf", "allOf"] {
        if let Some(choices) = schema[keyword].as_array() {
            let count = choices
                .iter()
                .filter(|s| validate(s, value).is_ok())
                .count();
            if match keyword {
                "anyOf" => count == 0,
                "oneOf" => count != 1,
                _ => count != choices.len(),
            } {
                return Err("argument alternatives mismatch");
            }
        }
    }
    if let Some(object) = value.as_object() {
        if schema["required"]
            .as_array()
            .is_some_and(|a| a.iter().any(|k| !object.contains_key(k.as_str().unwrap())))
        {
            return Err("required argument missing");
        }
        for (name, child) in object {
            if let Some(property) = schema["properties"].get(name) {
                validate(property, child)?;
            } else if schema["additionalProperties"] == false {
                return Err("unknown argument");
            } else if schema["additionalProperties"].is_object() {
                validate(&schema["additionalProperties"], child)?;
            }
        }
    }
    if let (Some(minimum), Some(values)) = (schema["minItems"].as_u64(), value.as_array()) {
        if (values.len() as u64) < minimum {
            return Err("too few array items");
        }
    }
    if let (Some(items), Some(values)) = (schema.get("items"), value.as_array()) {
        for child in values {
            validate(items, child)?;
        }
    }
    Ok(())
}

/// serde_json::Value accepts duplicate keys; tool records must not.
pub(super) fn parse_json(text: &str) -> Result<Value> {
    use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
    struct Unique(Value);
    impl<'de> Deserialize<'de> for Unique {
        fn deserialize<D: Deserializer<'de>>(
            deserializer: D,
        ) -> std::result::Result<Self, D::Error> {
            struct UniqueVisitor;
            impl<'de> Visitor<'de> for UniqueVisitor {
                type Value = Unique;
                fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                    f.write_str("JSON without duplicate keys")
                }
                fn visit_bool<E: de::Error>(self, v: bool) -> std::result::Result<Unique, E> {
                    Ok(Unique(v.into()))
                }
                fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<Unique, E> {
                    Ok(Unique(v.into()))
                }
                fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<Unique, E> {
                    Ok(Unique(v.into()))
                }
                fn visit_f64<E: de::Error>(self, v: f64) -> std::result::Result<Unique, E> {
                    Ok(Unique(v.into()))
                }
                fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<Unique, E> {
                    Ok(Unique(v.into()))
                }
                fn visit_unit<E: de::Error>(self) -> std::result::Result<Unique, E> {
                    Ok(Unique(Value::Null))
                }
                fn visit_seq<A: SeqAccess<'de>>(
                    self,
                    mut seq: A,
                ) -> std::result::Result<Unique, A::Error> {
                    let mut values = Vec::new();
                    while let Some(Unique(value)) = seq.next_element()? {
                        values.push(value);
                    }
                    Ok(Unique(values.into()))
                }
                fn visit_map<A: MapAccess<'de>>(
                    self,
                    mut map: A,
                ) -> std::result::Result<Unique, A::Error> {
                    let mut values = serde_json::Map::new();
                    while let Some((key, Unique(value))) = map.next_entry::<String, Unique>()? {
                        if values.insert(key, value).is_some() {
                            return Err(de::Error::custom("duplicate key"));
                        }
                    }
                    Ok(Unique(values.into()))
                }
            }
            deserializer.deserialize_any(UniqueVisitor)
        }
    }
    serde_json::from_str::<Unique>(text)
        .map(|value| value.0)
        .map_err(|_| "invalid or ambiguous JSON")
}
