use chrono::{DateTime, FixedOffset};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Deserialize)]
pub struct Template {
    pub target: String,
    pub table: String,
    pub fields: Vec<FieldDef>,
}

#[derive(Clone, Debug)]
pub struct Service {
    pub name: String,
    pub topic: String,
    pub templates: Vec<Template>,
}

#[derive(Deserialize)]
struct ServiceFile {
    name: String,
    topic: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FieldDef {
    pub name: String,
    pub kind: FieldKind,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub value_type: ValueType,
    #[serde(default)]
    pub validation: FieldValidation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldKind {
    Text,
    Select,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValueType {
    #[default]
    String,
    Boolean,
    Integer,
    Number,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct FieldValidation {
    #[serde(default)]
    pub min_length: Option<usize>,
    #[serde(default)]
    pub max_length: Option<usize>,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub max_digits: Option<usize>,
    #[serde(default)]
    pub max_integer_digits: Option<usize>,
    #[serde(default)]
    pub max_fraction_digits: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct FieldValue {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Insert,
    Update,
    Delete,
    Reload,
}

impl Action {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "INSERT" => Ok(Self::Insert),
            "UPDATE" => Ok(Self::Update),
            "DELETE" => Ok(Self::Delete),
            "RELOAD" => Ok(Self::Reload),
            _ => Err(format!("Action không hợp lệ: {value}")),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Insert => "INSERT",
            Self::Update => "UPDATE",
            Self::Delete => "DELETE",
            Self::Reload => "RELOAD",
        }
    }
}

pub fn load_templates(dir: &Path) -> Result<Vec<Template>, String> {
    let mut paths = fs::read_dir(dir)
        .map_err(|e| format!("Không đọc được thư mục {}: {e}", dir.display()))?
        .map(|entry| entry.map(|e| e.path()).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();

    let mut templates = Vec::new();
    let mut targets = HashSet::new();
    let mut tables = HashSet::new();
    for path in paths {
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let raw = fs::read_to_string(&path)
            .map_err(|e| format!("Không đọc được {}: {e}", path.display()))?;
        let template: Template = serde_json::from_str(&raw)
            .map_err(|e| format!("File {} không hợp lệ: {e}", path.display()))?;
        validate_template(&template).map_err(|e| format!("File {}: {e}", path.display()))?;
        if !targets.insert(template.target.clone()) {
            return Err(format!("Target trùng: {}", template.target));
        }
        if !tables.insert(template.table.clone()) {
            return Err(format!("Duplicate DB table: {}", template.table));
        }
        templates.push(template);
    }
    if templates.is_empty() {
        return Err(format!("Không có file JSON trong {}", dir.display()));
    }
    Ok(templates)
}

pub fn load_services(dir: &Path) -> Result<Vec<Service>, String> {
    let mut paths = fs::read_dir(dir)
        .map_err(|e| format!("Khong doc duoc thu muc {}: {e}", dir.display()))?
        .map(|entry| entry.map(|e| e.path()).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();

    let mut names = HashSet::new();
    let mut services = Vec::new();
    for path in paths.into_iter().filter(|path| path.is_dir()) {
        let raw = fs::read_to_string(path.join("service.json"))
            .map_err(|e| format!("Khong doc duoc service {}: {e}", path.display()))?;
        let metadata: ServiceFile = serde_json::from_str(&raw)
            .map_err(|e| format!("Service {} khong hop le: {e}", path.display()))?;
        if metadata.name.trim().is_empty() || !names.insert(metadata.name.clone()) {
            return Err(format!("Ten service trong hoac trung: {}", metadata.name));
        }
        if metadata.topic.trim().is_empty() {
            return Err(format!("Service {} chua co topic", path.display()));
        }
        let templates = load_templates(&path.join("templates"))
            .map_err(|e| format!("Service {}: {e}", path.display()))?;
        services.push(Service {
            name: metadata.name,
            topic: metadata.topic,
            templates,
        });
    }
    if services.is_empty() {
        return Err(format!("Khong co service trong {}", dir.display()));
    }
    Ok(services)
}

pub fn validate_template(template: &Template) -> Result<(), String> {
    if template.target.trim().is_empty() {
        return Err("target không được trống".into());
    }
    if template.table.trim().is_empty() {
        return Err("table must not be empty".into());
    }
    let mut names = HashSet::new();
    for field in &template.fields {
        if field.name.trim().is_empty() || !names.insert(&field.name) {
            return Err(format!("Tên trường trống hoặc trùng: {}", field.name));
        }
        if field.kind == FieldKind::Select
            && (field.options.is_empty()
                || field.options.iter().any(|option| option.trim().is_empty())
                || field.options.iter().collect::<HashSet<_>>().len() != field.options.len())
        {
            return Err(format!("Lựa chọn không hợp lệ cho trường {}", field.name));
        }
        if field.kind == FieldKind::Text && !field.options.is_empty() {
            return Err(format!("Trường nhập tay {} không cần options", field.name));
        }
        validate_field_definition(field)?;
    }
    Ok(())
}

fn validate_field_definition(field: &FieldDef) -> Result<(), String> {
    let validation = &field.validation;
    if validation.min_length > validation.max_length || validation.min > validation.max {
        return Err(format!(
            "Giới hạn min/max không hợp lệ cho trường {}",
            field.name
        ));
    }
    if [
        validation.max_digits,
        validation.max_integer_digits,
        validation.max_fraction_digits,
    ]
    .into_iter()
    .flatten()
    .any(|limit| limit == 0)
    {
        return Err(format!(
            "Giới hạn chữ số phải lớn hơn 0 cho trường {}",
            field.name
        ));
    }

    match field.value_type {
        ValueType::String => {
            if validation.min.is_some()
                || validation.max.is_some()
                || validation.max_digits.is_some()
                || validation.max_integer_digits.is_some()
                || validation.max_fraction_digits.is_some()
            {
                return Err(format!(
                    "Validation số không áp dụng cho trường chuỗi {}",
                    field.name
                ));
            }
        }
        ValueType::Boolean => {
            if validation.min_length.is_some()
                || validation.max_length.is_some()
                || validation.min.is_some()
                || validation.max.is_some()
                || validation.max_digits.is_some()
                || validation.max_integer_digits.is_some()
                || validation.max_fraction_digits.is_some()
            {
                return Err(format!(
                    "Boolean {} không hỗ trợ validation bổ sung",
                    field.name
                ));
            }
            if field.kind == FieldKind::Select
                && field
                    .options
                    .iter()
                    .any(|option| option != "true" && option != "false")
            {
                return Err(format!(
                    "Lựa chọn boolean không hợp lệ cho trường {}",
                    field.name
                ));
            }
        }
        ValueType::Integer => {
            if validation.min_length.is_some()
                || validation.max_length.is_some()
                || validation.max_integer_digits.is_some()
                || validation.max_fraction_digits.is_some()
            {
                return Err(format!(
                    "Validation không hợp lệ cho trường số nguyên {}",
                    field.name
                ));
            }
            for bound in [validation.min, validation.max].into_iter().flatten() {
                if !bound.is_finite() || bound.fract() != 0.0 {
                    return Err(format!(
                        "Giới hạn số nguyên không hợp lệ cho trường {}",
                        field.name
                    ));
                }
            }
        }
        ValueType::Number => {
            if validation.min_length.is_some()
                || validation.max_length.is_some()
                || validation.max_digits.is_some()
            {
                return Err(format!(
                    "Validation không hợp lệ cho trường số {}",
                    field.name
                ));
            }
            if [validation.min, validation.max]
                .into_iter()
                .flatten()
                .any(|bound| !bound.is_finite())
            {
                return Err(format!(
                    "Giới hạn số không hợp lệ cho trường {}",
                    field.name
                ));
            }
        }
    }
    Ok(())
}

pub fn build_message(
    template: &Template,
    action: Action,
    fields: &[FieldValue],
    changed_at: DateTime<FixedOffset>,
) -> Result<String, String> {
    render_message(template, action, fields, changed_at, true)
}

pub fn preview_message(
    template: &Template,
    action: Action,
    fields: &[FieldValue],
    changed_at: DateTime<FixedOffset>,
) -> Result<String, String> {
    render_message(template, action, fields, changed_at, false)
}

pub fn validate_field_value(definition: &FieldDef, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Ok(());
    }
    if definition.kind == FieldKind::Select
        && !definition.options.iter().any(|option| option == value)
    {
        return Err(format!("Giá trị không hợp lệ cho {}", definition.name));
    }
    parse_field_value(definition, value).map(|_| ())
}

pub fn validate_field_input(definition: &FieldDef, value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Ok(());
    }
    match definition.value_type {
        ValueType::String => {
            if definition
                .validation
                .max_length
                .is_some_and(|max| value.chars().count() > max)
            {
                Err(format!(
                    "Độ dài giá trị cho {} vượt giới hạn",
                    definition.name
                ))
            } else {
                Ok(())
            }
        }
        ValueType::Boolean => {
            if "true".starts_with(value) || "false".starts_with(value) {
                Ok(())
            } else {
                Err(format!(
                    "Giá trị cho {} chỉ nhận true hoặc false",
                    definition.name
                ))
            }
        }
        ValueType::Integer => {
            if value == "-" {
                return Ok(());
            }
            let digits = value.strip_prefix('-').unwrap_or(value);
            if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(format!(
                    "Giá trị cho {} chỉ nhận số nguyên",
                    definition.name
                ));
            }
            if definition
                .validation
                .max_digits
                .is_some_and(|max| digits.len() > max)
            {
                return Err(format!(
                    "Giá trị cho {} vượt số chữ số cho phép",
                    definition.name
                ));
            }
            Ok(())
        }
        ValueType::Number => {
            let unsigned = value.strip_prefix('-').unwrap_or(value);
            if value == "-" {
                return Ok(());
            }
            let Some((integer, fraction)) = unsigned
                .split_once('.')
                .map_or(Some((unsigned, None)), |(integer, fraction)| {
                    Some((integer, Some(fraction)))
                })
            else {
                return Err(format!("Giá trị cho {} phải là số", definition.name));
            };
            if integer.is_empty()
                || !integer.bytes().all(|byte| byte.is_ascii_digit())
                || fraction.is_some_and(|part| !part.bytes().all(|byte| byte.is_ascii_digit()))
            {
                return Err(format!(
                    "Giá trị cho {} chỉ nhận chữ số, '-' và '.'",
                    definition.name
                ));
            }
            if definition
                .validation
                .max_integer_digits
                .is_some_and(|max| integer.len() > max)
                || definition
                    .validation
                    .max_fraction_digits
                    .is_some_and(|max| fraction.is_some_and(|part| part.len() > max))
            {
                return Err(format!(
                    "Giá trị cho {} vượt số chữ số cho phép",
                    definition.name
                ));
            }
            Ok(())
        }
    }
}

/// Validates text currently being edited without rejecting incomplete numeric input such as `-`.
pub fn validate_field_entry(definition: &FieldDef, value: &str) -> Result<(), String> {
    validate_field_input(definition, value)?;
    if matches!(
        definition.value_type,
        ValueType::Integer | ValueType::Number
    ) && (value == "-" || (definition.value_type == ValueType::Number && value.ends_with('.')))
    {
        return Ok(());
    }
    validate_field_value(definition, value)
}

pub fn beautify_json(payload: &str) -> Result<String, String> {
    let value: Value = serde_json::from_str(payload).map_err(|error| error.to_string())?;
    serde_json::to_string_pretty(&value).map_err(|error| error.to_string())
}

fn render_message(
    template: &Template,
    action: Action,
    fields: &[FieldValue],
    changed_at: DateTime<FixedOffset>,
    require_values: bool,
) -> Result<String, String> {
    let mut data = Map::new();
    for field in fields.iter().filter(|field| !field.value.trim().is_empty()) {
        let definition = template
            .fields
            .iter()
            .find(|definition| definition.name == field.name)
            .ok_or_else(|| format!("Trường không có trong form: {}", field.name))?;
        if require_values && field.value.trim().is_empty() {
            return Err(format!("Cần nhập giá trị cho {}", field.name));
        }
        validate_field_value(definition, &field.value)?;
        data.insert(
            field.name.clone(),
            parse_field_value(definition, &field.value)?,
        );
    }
    let data = if data.is_empty() {
        Value::Null
    } else {
        Value::Object(data)
    };
    serde_json::to_string_pretty(&json!({
        "target": &template.target,
        "action": action.as_str(),
        "data": data,
        "changedAt": changed_at.to_rfc3339(),
    }))
    .map_err(|e| e.to_string())
}

fn parse_field_value(definition: &FieldDef, value: &str) -> Result<Value, String> {
    let field_name = &definition.name;
    let validation = &definition.validation;
    match definition.value_type {
        ValueType::String => {
            let length = value.chars().count();
            if validation.min_length.is_some_and(|min| length < min)
                || validation.max_length.is_some_and(|max| length > max)
            {
                return Err(format!("Độ dài giá trị cho {field_name} không hợp lệ"));
            }
            Ok(Value::String(value.into()))
        }
        ValueType::Boolean => value
            .parse::<bool>()
            .map(Value::Bool)
            .map_err(|_| format!("Giá trị cho {field_name} phải là boolean (true hoặc false)")),
        ValueType::Integer => {
            let number = value
                .parse::<i64>()
                .map_err(|_| format!("Giá trị cho {field_name} phải là số nguyên"))?;
            let digits = value.strip_prefix('-').unwrap_or(value).len();
            if validation.max_digits.is_some_and(|max| digits > max)
                || validation.min.is_some_and(|min| (number as f64) < min)
                || validation.max.is_some_and(|max| (number as f64) > max)
            {
                return Err(format!("Giá trị cho {field_name} vượt giới hạn template"));
            }
            Ok(Value::from(number))
        }
        ValueType::Number => {
            let (integer_digits, fraction_digits) = decimal_digit_counts(value)
                .ok_or_else(|| format!("Giá trị cho {field_name} phải là số thập phân hợp lệ"))?;
            let number = value
                .parse::<f64>()
                .ok()
                .and_then(serde_json::Number::from_f64)
                .ok_or_else(|| format!("Giá trị cho {field_name} phải là số"))?;
            let numeric_value = number.as_f64().expect("finite JSON number");
            if validation
                .max_integer_digits
                .is_some_and(|max| integer_digits > max)
                || validation
                    .max_fraction_digits
                    .is_some_and(|max| fraction_digits > max)
                || validation.min.is_some_and(|min| numeric_value < min)
                || validation.max.is_some_and(|max| numeric_value > max)
            {
                return Err(format!("Giá trị cho {field_name} vượt giới hạn template"));
            }
            Ok(Value::Number(number))
        }
    }
}

fn decimal_digit_counts(value: &str) -> Option<(usize, usize)> {
    let unsigned = value.strip_prefix('-').unwrap_or(value);
    let (integer, fraction) = unsigned
        .split_once('.')
        .map_or((unsigned, None), |(integer, fraction)| {
            (integer, Some(fraction))
        });
    if integer.is_empty()
        || (integer.len() > 1 && integer.starts_with('0'))
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.is_some_and(|fraction| {
            fraction.is_empty() || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        })
    {
        return None;
    }
    Some((integer.len(), fraction.map_or(0, str::len)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static FIXTURE_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct ServiceFixture {
        path: std::path::PathBuf,
    }

    impl Drop for ServiceFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn service_fixture(services: &[(&str, &str, &str, &[&str])]) -> ServiceFixture {
        let path = std::env::temp_dir().join(format!(
            "kafka-message-tool-service-test-{}-{}",
            std::process::id(),
            FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        for (id, name, topic, targets) in services {
            let service_dir = path.join(id);
            let templates = service_dir.join("templates");
            fs::create_dir_all(&templates).unwrap();
            fs::write(
                service_dir.join("service.json"),
                json!({ "name": name, "topic": topic }).to_string(),
            )
            .unwrap();
            for (index, target) in targets.iter().enumerate() {
                fs::write(
                    templates.join(format!("{index}.json")),
                    json!({ "target": target, "table": format!("{id}_{index}"), "fields": [] })
                        .to_string(),
                )
                .unwrap();
            }
        }
        ServiceFixture { path }
    }

    fn template() -> Template {
        serde_json::from_str(include_str!(
            "../services/cache/templates/channel-config.json"
        ))
        .unwrap()
    }

    fn time() -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339("2026-04-10T15:30:00.7484588+07:00").unwrap()
    }

    fn field(name: &str, value: &str) -> FieldValue {
        FieldValue {
            name: name.into(),
            value: value.into(),
        }
    }

    fn data(payload: &str) -> Value {
        serde_json::from_str::<Value>(payload).unwrap()["data"].clone()
    }

    #[test]
    fn beautifies_valid_json_for_manual_messages() {
        assert_eq!(
            beautify_json("{\"target\":\"cache\",\"enabled\":true}").unwrap(),
            "{\n  \"enabled\": true,\n  \"target\": \"cache\"\n}"
        );
    }

    #[test]
    fn rejects_invalid_json_for_manual_message_formatting() {
        assert!(beautify_json("plain text").is_err());
    }

    #[test]
    fn null_data_when_all_fields_are_empty() {
        assert_eq!(
            data(&build_message(&template(), Action::Reload, &[], time()).unwrap()),
            Value::Null
        );
        assert_eq!(
            data(
                &build_message(&template(), Action::Reload, &[field("code", " ")], time(),)
                    .unwrap()
            ),
            Value::Null
        );
    }

    #[test]
    fn one_and_multiple_selected_fields() {
        let one = build_message(
            &template(),
            Action::Reload,
            &[field("type", "FAST")],
            time(),
        )
        .unwrap();
        assert_eq!(data(&one), json!({"type": "FAST"}));
        let two = build_message(
            &template(),
            Action::Reload,
            &[field("code", "abc"), field("type", "SLOW")],
            time(),
        )
        .unwrap();
        assert_eq!(data(&two), json!({"code": "abc", "type": "SLOW"}));
    }

    #[test]
    fn omits_empty_fields_and_keeps_entered_fields() {
        let payload = build_message(
            &template(),
            Action::Reload,
            &[field("code", ""), field("type", "FAST")],
            time(),
        )
        .unwrap();
        assert_eq!(data(&payload), json!({"type": "FAST"}));
    }

    #[test]
    fn rejects_invalid_selected_value() {
        assert!(
            build_message(
                &template(),
                Action::Reload,
                &[field("type", "MEDIUM")],
                time()
            )
            .is_err()
        );
    }

    #[test]
    fn renders_boolean_and_numeric_template_values_as_json_primitives() {
        let template: Template = serde_json::from_str(
            r#"{
                "target":"FeatureFlag",
                "table":"feature_flags",
                "fields":[
                    {"name":"enabled","kind":"select","options":["true","false"],"value_type":"boolean"},
                    {"name":"priority","kind":"text","value_type":"integer"},
                    {"name":"ratio","kind":"text","value_type":"number"}
                ]
            }"#,
        )
        .unwrap();

        let payload = build_message(
            &template,
            Action::Update,
            &[
                field("enabled", "true"),
                field("priority", "2"),
                field("ratio", "1.5"),
            ],
            time(),
        )
        .unwrap();

        assert_eq!(
            data(&payload),
            json!({"enabled": true, "priority": 2, "ratio": 1.5})
        );
    }

    #[test]
    fn rejects_value_that_does_not_match_its_template_primitive_type() {
        let template: Template = serde_json::from_str(
            r#"{
                "target":"FeatureFlag",
                "table":"feature_flags",
                "fields":[{"name":"enabled","kind":"text","value_type":"boolean"}]
            }"#,
        )
        .unwrap();

        let error = build_message(
            &template,
            Action::Update,
            &[field("enabled", "yes")],
            time(),
        )
        .unwrap_err();

        assert!(error.contains("enabled"));
    }

    #[test]
    fn rejects_string_longer_than_template_maximum_length() {
        let template: Template = serde_json::from_str(
            r#"{
                "target":"ValidationExample",
                "table":"validation_examples",
                "fields":[{"name":"label","kind":"text","validation":{"max_length":4}}]
            }"#,
        )
        .unwrap();

        assert!(
            build_message(&template, Action::Update, &[field("label", "long")], time()).is_ok()
        );
        assert!(
            build_message(
                &template,
                Action::Update,
                &[field("label", "longer")],
                time()
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_integer_outside_template_limits() {
        let template: Template = serde_json::from_str(
            r#"{
                "target":"ValidationExample",
                "table":"validation_examples",
                "fields":[{"name":"count","kind":"text","value_type":"integer","validation":{"min":0,"max":99,"max_digits":2}}]
            }"#,
        )
        .unwrap();

        assert!(build_message(&template, Action::Update, &[field("count", "99")], time()).is_ok());
        assert!(
            build_message(&template, Action::Update, &[field("count", "100")], time()).is_err()
        );
        assert!(build_message(&template, Action::Update, &[field("count", "-1")], time()).is_err());
    }

    #[test]
    fn rejects_number_that_exceeds_template_integer_or_fraction_digits() {
        let template: Template = serde_json::from_str(
            r#"{
                "target":"ValidationExample",
                "table":"validation_examples",
                "fields":[{"name":"amount","kind":"text","value_type":"number","validation":{"max_integer_digits":3,"max_fraction_digits":2}}]
            }"#,
        )
        .unwrap();

        assert!(
            build_message(
                &template,
                Action::Update,
                &[field("amount", "123.45")],
                time()
            )
            .is_ok()
        );
        assert!(
            build_message(
                &template,
                Action::Update,
                &[field("amount", "1234.5")],
                time()
            )
            .is_err()
        );
        assert!(
            build_message(
                &template,
                Action::Update,
                &[field("amount", "12.345")],
                time()
            )
            .is_err()
        );
    }

    #[test]
    fn validates_a_single_field_for_immediate_form_feedback() {
        let template: Template = serde_json::from_str(
            r#"{
                "target":"ValidationExample",
                "table":"validation_examples",
                "fields":[{"name":"amount","kind":"text","value_type":"number","validation":{"max_integer_digits":3,"max_fraction_digits":2}}]
            }"#,
        )
        .unwrap();
        let definition = &template.fields[0];

        assert!(validate_field_value(definition, "").is_ok());
        assert!(validate_field_value(definition, "12.34").is_ok());
        assert!(validate_field_value(definition, "1234.56").is_err());
    }

    #[test]
    fn rejects_letters_and_excess_digits_while_entering_a_number() {
        let template: Template = serde_json::from_str(
            r#"{
                "target":"ValidationExample",
                "table":"validation_examples",
                "fields":[{"name":"amount","kind":"text","value_type":"number","validation":{"max_integer_digits":3,"max_fraction_digits":2}}]
            }"#,
        )
        .unwrap();
        let definition = &template.fields[0];

        assert!(validate_field_input(definition, "12.").is_ok());
        assert!(validate_field_input(definition, "12a").is_err());
        assert!(validate_field_input(definition, "1234").is_err());
        assert!(validate_field_input(definition, "12.345").is_err());
    }

    #[test]
    fn accepts_incomplete_negative_number_while_editing() {
        let template: Template = serde_json::from_str(
            r#"{
                "target":"ValidationExample",
                "table":"validation_examples",
                "fields":[{"name":"amount","kind":"text","value_type":"number","validation":{"min":-99.99,"max_integer_digits":2,"max_fraction_digits":2}}]
            }"#,
        )
        .unwrap();
        let definition = &template.fields[0];

        assert!(validate_field_entry(definition, "-").is_ok());
        assert!(validate_field_entry(definition, "-12.").is_ok());
        assert!(validate_field_entry(definition, "-12.34").is_ok());
    }

    #[test]
    fn preview_omits_empty_fields() {
        let fields = [field("code", "")];
        let preview = preview_message(&template(), Action::Reload, &fields, time()).unwrap();
        assert_eq!(data(&preview), Value::Null);
    }

    #[test]
    fn preserves_action_and_offset() {
        let value: Value =
            serde_json::from_str(&build_message(&template(), Action::Delete, &[], time()).unwrap())
                .unwrap();
        assert_eq!(value["target"], "ChannelConfig");
        assert_eq!(value["action"], "DELETE");
        assert_eq!(value["changedAt"], "2026-04-10T15:30:00.748458800+07:00");
    }

    #[test]
    fn rejects_invalid_template() {
        let mut value = template();
        value.fields[1].options.clear();
        assert!(validate_template(&value).is_err());
    }

    #[test]
    fn rejects_template_without_table_metadata() {
        assert!(
            serde_json::from_str::<Template>(r#"{"target":"ChannelConfig","fields":[]}"#).is_err()
        );
    }

    #[test]
    fn allows_a_target_name_in_different_services() {
        let root = service_fixture(&[
            ("cache", "Cache", "cache-topic", &["ChannelConfig"]),
            (
                "notification",
                "Notification",
                "notification-topic",
                &["ChannelConfig"],
            ),
        ]);
        assert_eq!(load_services(&root.path).unwrap().len(), 2);
    }

    #[test]
    fn rejects_duplicate_targets_in_one_service() {
        let root = service_fixture(&[(
            "cache",
            "Cache",
            "cache-topic",
            &["ChannelConfig", "ChannelConfig"],
        )]);
        assert!(load_services(&root.path).is_err());
    }

    #[test]
    fn rejects_a_service_without_a_topic() {
        let root = service_fixture(&[("cache", "Cache", "", &["ChannelConfig"])]);
        assert!(load_services(&root.path).is_err());
    }
}
