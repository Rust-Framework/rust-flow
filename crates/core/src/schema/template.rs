//! Simple `{{label}}` / `{{data.key}}` template substitution.

/// Apply templates against node label and data object.
pub fn apply_template(template: &str, label: &str, data: &serde_json::Value) -> String {
    let mut out = template.replace("{{label}}", label);

    if let Some(obj) = data.as_object() {
        for (key, value) in obj {
            let placeholder = format!("{{{{data.{key}}}}}");
            let replacement = json_value_to_string(value);
            out = out.replace(&placeholder, &replacement);
        }
    }

  // Remaining {{data.x}} for nested paths (single level)
    while let Some(start) = out.find("{{data.") {
        if let Some(end) = out[start..].find("}}") {
            let key = out[start + 7..start + end].trim();
            let placeholder = &out[start..start + end + 2];
            let value = data.get(key).map(json_value_to_string).unwrap_or_default();
            out = out.replace(placeholder, &value);
        } else {
            break;
        }
    }

    out
}

fn json_value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_label_and_data() {
        let data = serde_json::json!({ "condition": "x > 0", "iterator": "item" });
        assert_eq!(
            apply_template("{{label}}", "Start", &data),
            "Start"
        );
        assert_eq!(
            apply_template("if ({{data.condition}})", "If", &data),
            "if (x > 0)"
        );
        assert_eq!(
            apply_template("for {{data.iterator}} in {{data.collection}}", "Loop", &serde_json::json!({"iterator":"i","collection":"items"})),
            "for i in items"
        );
    }
}
