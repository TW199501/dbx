use serde::de::{self, IgnoredAny, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;
use std::marker::PhantomData;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CliSource {
    GuiRuntime,
    Headless,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CliErrorCode {
    GuiRuntimeRequired,
    ConnectionNotFound,
    AmbiguousConnection,
    SecretUnavailable,
    SshTunnelFailed,
    QueryClassificationFailed,
    HandoffRequired,
    DdlBlocked,
    ProductionWriteBlocked,
    UnsupportedDatabaseType,
    Timeout,
    InternalError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliError {
    pub code: CliErrorCode,
    pub message: String,
    pub recoverable: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum CliEnvelope<T> {
    Success { ok: bool, source: CliSource, data: T },
    Failure { ok: bool, source: CliSource, error: CliError },
}

impl<'de, T> Deserialize<'de> for CliEnvelope<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(CliEnvelopeVisitor { marker: PhantomData })
    }
}

struct CliEnvelopeVisitor<T> {
    marker: PhantomData<T>,
}

impl<'de, T> Visitor<'de> for CliEnvelopeVisitor<T>
where
    T: Deserialize<'de>,
{
    type Value = CliEnvelope<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a CLI envelope with consistent ok/data/error fields")
    }

    fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut ok = None;
        let mut source = None;
        let mut data = None;
        let mut has_data = false;
        let mut error = None;
        let mut has_error = false;

        while let Some(key) = map.next_key::<CliEnvelopeField>()? {
            match key {
                CliEnvelopeField::Ok => {
                    if ok.is_some() {
                        return Err(de::Error::duplicate_field("ok"));
                    }
                    ok = Some(map.next_value()?);
                }
                CliEnvelopeField::Source => {
                    if source.is_some() {
                        return Err(de::Error::duplicate_field("source"));
                    }
                    source = Some(map.next_value()?);
                }
                CliEnvelopeField::Data => {
                    if has_data {
                        return Err(de::Error::duplicate_field("data"));
                    }
                    has_data = true;
                    data = Some(map.next_value()?);
                }
                CliEnvelopeField::Error => {
                    if has_error {
                        return Err(de::Error::duplicate_field("error"));
                    }
                    has_error = true;
                    error = Some(map.next_value()?);
                }
                CliEnvelopeField::Ignore => {
                    let _ = map.next_value::<IgnoredAny>()?;
                }
            }
        }

        let ok = ok.ok_or_else(|| de::Error::missing_field("ok"))?;
        let source = source.ok_or_else(|| de::Error::missing_field("source"))?;

        match (ok, has_data, has_error) {
            (true, true, false) => Ok(CliEnvelope::Success { ok, source, data: data.expect("data presence was checked") }),
            (false, false, true) => Ok(CliEnvelope::Failure { ok, source, error: error.expect("error presence was checked") }),
            (true, _, _) => Err(de::Error::custom("ok=true requires data without error")),
            (false, _, _) => Err(de::Error::custom("ok=false requires error without data")),
        }
    }
}

enum CliEnvelopeField {
    Ok,
    Source,
    Data,
    Error,
    Ignore,
}

impl<'de> Deserialize<'de> for CliEnvelopeField {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_identifier(CliEnvelopeFieldVisitor)
    }
}

struct CliEnvelopeFieldVisitor;

impl<'de> Visitor<'de> for CliEnvelopeFieldVisitor {
    type Value = CliEnvelopeField;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a CLI envelope field")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(match value {
            "ok" => CliEnvelopeField::Ok,
            "source" => CliEnvelopeField::Source,
            "data" => CliEnvelopeField::Data,
            "error" => CliEnvelopeField::Error,
            _ => CliEnvelopeField::Ignore,
        })
    }
}

pub fn ok<T>(source: CliSource, data: T) -> CliEnvelope<T> {
    CliEnvelope::Success { ok: true, source, data }
}

pub fn fail<T>(source: CliSource, code: CliErrorCode, message: impl Into<String>, recoverable: bool) -> CliEnvelope<T> {
    CliEnvelope::Failure { ok: false, source, error: CliError { code, message: message.into(), recoverable } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_success_source_as_kebab_case() {
        let env = ok(CliSource::GuiRuntime, serde_json::json!({"value": 1}));
        let json = serde_json::to_string(&env).unwrap();
        assert!(json.contains("\"ok\":true"));
        assert!(json.contains("\"source\":\"gui-runtime\""));
    }

    #[test]
    fn serializes_error_code_as_screaming_snake_case() {
        let env: CliEnvelope<()> = fail(CliSource::Headless, CliErrorCode::GuiRuntimeRequired, "runtime needed", true);
        let json = serde_json::to_string(&env).unwrap();
        assert!(json.contains("\"GUI_RUNTIME_REQUIRED\""));
    }

    #[test]
    fn deserializes_success_when_ok_true_and_data_present() {
        let env: CliEnvelope<serde_json::Value> = serde_json::from_value(serde_json::json!({
            "ok": true,
            "source": "gui-runtime",
            "data": { "value": 1 }
        }))
        .unwrap();

        match env {
            CliEnvelope::Success { ok, source, data } => {
                assert!(ok);
                assert_eq!(source, CliSource::GuiRuntime);
                assert_eq!(data, serde_json::json!({ "value": 1 }));
            }
            CliEnvelope::Failure { .. } => panic!("expected success envelope"),
        }
    }

    #[test]
    fn deserializes_failure_when_ok_false_and_error_present() {
        let env: CliEnvelope<serde_json::Value> = serde_json::from_value(serde_json::json!({
            "ok": false,
            "source": "headless",
            "error": {
                "code": "GUI_RUNTIME_REQUIRED",
                "message": "runtime needed",
                "recoverable": true
            }
        }))
        .unwrap();

        match env {
            CliEnvelope::Failure { ok, source, error } => {
                assert!(!ok);
                assert_eq!(source, CliSource::Headless);
                assert_eq!(error.code, CliErrorCode::GuiRuntimeRequired);
                assert_eq!(error.message, "runtime needed");
                assert!(error.recoverable);
            }
            CliEnvelope::Success { .. } => panic!("expected failure envelope"),
        }
    }

    #[test]
    fn rejects_success_shape_when_ok_is_false() {
        let err = serde_json::from_value::<CliEnvelope<serde_json::Value>>(serde_json::json!({
            "ok": false,
            "source": "headless",
            "data": { "runtime": "headless" }
        }))
        .unwrap_err();

        assert!(err.to_string().contains("ok=false requires error without data"));
    }

    #[test]
    fn rejects_failure_shape_when_ok_is_true() {
        let err = serde_json::from_value::<CliEnvelope<serde_json::Value>>(serde_json::json!({
            "ok": true,
            "source": "headless",
            "error": {
                "code": "GUI_RUNTIME_REQUIRED",
                "message": "runtime needed",
                "recoverable": true
            }
        }))
        .unwrap_err();

        assert!(err.to_string().contains("ok=true requires data without error"));
    }

    #[test]
    fn rejects_envelope_with_both_data_and_error() {
        let err = serde_json::from_value::<CliEnvelope<serde_json::Value>>(serde_json::json!({
            "ok": true,
            "source": "gui-runtime",
            "data": { "value": 1 },
            "error": {
                "code": "INTERNAL_ERROR",
                "message": "unexpected",
                "recoverable": false
            }
        }))
        .unwrap_err();

        assert!(err.to_string().contains("ok=true requires data without error"));
    }
}
