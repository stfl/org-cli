/// CLI output envelope.
///
/// Every command emits one of:
///   Success: {"ok": true, "data": <value>}
///   Error:   {"ok": false, "error": {...}, "exit_code": <int>}
///
/// Default output is pretty-printed JSON; --compact emits a single line.
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct SuccessEnvelope {
    pub ok: bool,
    pub data: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorEnvelope {
    pub ok: bool,
    pub error: ErrorDetail,
    pub exit_code: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorDetail {
    pub kind: ErrorKind,
    pub code: i64,
    pub message: String,
    pub data: Value,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ErrorKind {
    Tool,
    Transport,
    Usage,
}

/// Print a success envelope to stdout.
pub fn print_success(data: Value, compact: bool) {
    let env = SuccessEnvelope { ok: true, data };
    print_json(&serde_json::to_value(env).unwrap(), compact);
}

/// Print an error envelope to stdout and return the exit code.
pub fn print_error(
    kind: ErrorKind,
    code: i64,
    message: String,
    data: Value,
    exit_code: i32,
    compact: bool,
) -> i32 {
    let env = ErrorEnvelope {
        ok: false,
        error: ErrorDetail {
            kind,
            code,
            message,
            data,
        },
        exit_code,
    };
    print_json(&serde_json::to_value(env).unwrap(), compact);
    exit_code
}

fn print_json(v: &Value, compact: bool) {
    if compact {
        println!("{}", serde_json::to_string(v).unwrap());
    } else {
        println!("{}", serde_json::to_string_pretty(v).unwrap());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_success_envelope_serialization() {
        let env = SuccessEnvelope {
            ok: true,
            data: json!({"tools": []}),
        };
        let v = serde_json::to_value(&env).unwrap();
        assert_eq!(v["ok"], true);
        assert!(v["data"]["tools"].is_array());
        assert!(v.get("error").is_none());
    }

    #[test]
    fn test_error_envelope_serialization() {
        let env = ErrorEnvelope {
            ok: false,
            error: ErrorDetail {
                kind: ErrorKind::Tool,
                code: -32000,
                message: "Invalid input".to_string(),
                data: json!(null),
            },
            exit_code: 1,
        };
        let v = serde_json::to_value(&env).unwrap();
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"]["kind"], "tool");
        assert_eq!(v["error"]["code"], -32000);
        assert_eq!(v["exit_code"], 1);
        assert!(v.get("data").is_none());
    }

    #[test]
    fn test_error_kind_serialization() {
        assert_eq!(serde_json::to_string(&ErrorKind::Tool).unwrap(), "\"tool\"");
        assert_eq!(
            serde_json::to_string(&ErrorKind::Transport).unwrap(),
            "\"transport\""
        );
        assert_eq!(
            serde_json::to_string(&ErrorKind::Usage).unwrap(),
            "\"usage\""
        );
    }
}
