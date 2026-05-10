use dbx_core::cli::{fail, ok, CliEnvelope, CliErrorCode, CliSource};

pub(crate) async fn run(args: Vec<String>) -> Result<(), CliEnvelope<()>> {
    let output = dispatch(args).await;
    println!("{}", serde_json::to_string_pretty(&output).unwrap());

    if matches!(output, CliEnvelope::Failure { .. }) {
        std::process::exit(1);
    }

    Ok(())
}

pub(crate) async fn dispatch(args: Vec<String>) -> CliEnvelope<serde_json::Value> {
    let parsed = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(err) => return err,
    };

    match parsed.positionals.as_slice() {
        [cmd, rest @ ..] if cmd == "context" => context(rest).await,
        [cmd, sub, rest @ ..] if cmd == "conn" && sub == "list" => conn_list(rest).await,
        [cmd, sub, name, rest @ ..] if cmd == "conn" && sub == "show" => conn_show(name, rest).await,
        [cmd, sub, rest @ ..] if cmd == "schema" && sub == "snapshot" => schema_snapshot(rest).await,
        [cmd, rest @ ..] if cmd == "safe-query" => safe_query(rest).await,
        [cmd, rest @ ..] if cmd == "handoff" => handoff(rest).await,
        [cmd, rest @ ..] if cmd == "selection" => selection(rest).await,
        [cmd, sub, rest @ ..] if cmd == "result" && sub == "current" => result_current(rest).await,
        _ => fail(CliSource::Headless, CliErrorCode::InternalError, "Unknown command", false),
    }
}

struct ParsedArgs {
    positionals: Vec<String>,
}

fn parse_args(args: Vec<String>) -> Result<ParsedArgs, CliEnvelope<serde_json::Value>> {
    let mut positionals = Vec::new();
    let mut index = 0;

    while index < args.len() {
        if args[index] == "--format" {
            let Some(format) = args.get(index + 1) else {
                return Err(fail(CliSource::Headless, CliErrorCode::InternalError, "--format requires a value", true));
            };
            if format != "json" {
                return Err(fail(CliSource::Headless, CliErrorCode::InternalError, "Only --format json is supported", true));
            }
            index += 2;
        } else {
            positionals.push(args[index].clone());
            index += 1;
        }
    }

    Ok(ParsedArgs { positionals })
}

async fn context(_args: &[String]) -> CliEnvelope<serde_json::Value> {
    match crate::runtime_client::get_json("/context").await {
        Ok(data) => ok(CliSource::GuiRuntime, data),
        Err(_) => ok(CliSource::Headless, serde_json::json!({ "runtime": "headless" })),
    }
}

async fn conn_list(_args: &[String]) -> CliEnvelope<serde_json::Value> {
    ok(CliSource::Headless, serde_json::json!({ "connections": [] }))
}

async fn conn_show(_name: &str, _args: &[String]) -> CliEnvelope<serde_json::Value> {
    ok(CliSource::Headless, serde_json::json!({}))
}

async fn schema_snapshot(args: &[String]) -> CliEnvelope<serde_json::Value> {
    let Some(_conn) = option_value(args, "--conn") else {
        return fail(CliSource::Headless, CliErrorCode::ConnectionNotFound, "--conn is required", true);
    };

    fail(CliSource::Headless, CliErrorCode::InternalError, "Schema snapshot headless execution is not implemented", true)
}

async fn safe_query(args: &[String]) -> CliEnvelope<serde_json::Value> {
    let Some(_conn) = option_value(args, "--conn") else {
        return fail(CliSource::Headless, CliErrorCode::ConnectionNotFound, "--conn is required", true);
    };
    let Some(_sql) = option_value(args, "--sql") else {
        return fail(CliSource::Headless, CliErrorCode::QueryClassificationFailed, "--sql is required", true);
    };

    fail(CliSource::Headless, CliErrorCode::InternalError, "Safe query headless execution is not implemented", true)
}

async fn handoff(args: &[String]) -> CliEnvelope<serde_json::Value> {
    let body = serde_json::json!({
        "conn": option_value(args, "--conn"),
        "title": option_value(args, "--title"),
        "sql": option_value(args, "--sql"),
    });

    match crate::runtime_client::post_json("/handoff", body).await {
        Ok(data) => ok(CliSource::GuiRuntime, data),
        Err(_) => fail(CliSource::Headless, CliErrorCode::InternalError, "Handoff payload is required", true),
    }
}

async fn selection(_args: &[String]) -> CliEnvelope<serde_json::Value> {
    match crate::runtime_client::get_json("/selection").await {
        Ok(data) => ok(CliSource::GuiRuntime, data),
        Err(_) => runtime_required("dbx selection requires DBX GUI runtime."),
    }
}

async fn result_current(args: &[String]) -> CliEnvelope<serde_json::Value> {
    let limit = option_value(args, "--limit").unwrap_or("50");

    match crate::runtime_client::get_json(&format!("/result/current?limit={limit}")).await {
        Ok(data) => ok(CliSource::GuiRuntime, data),
        Err(_) => runtime_required("dbx result current requires DBX GUI runtime."),
    }
}

fn runtime_required(message: &str) -> CliEnvelope<serde_json::Value> {
    fail(CliSource::Headless, CliErrorCode::GuiRuntimeRequired, message, true)
}

fn option_value<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|pair| pair[0] == key)
        .map(|pair| pair[1].as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dbx_core::cli::CliErrorCode;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn assert_failure_code(env: CliEnvelope<serde_json::Value>, expected: CliErrorCode) {
        match env {
            CliEnvelope::Failure { error, .. } => assert_eq!(error.code, expected),
            CliEnvelope::Success { .. } => panic!("expected failure envelope"),
        }
    }

    #[tokio::test]
    async fn gui_only_commands_return_runtime_required_without_runtime() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("DBX_APP_DATA_DIR", dir.path());

        assert_failure_code(
            dispatch(vec!["selection".into(), "--format".into(), "json".into()]).await,
            CliErrorCode::GuiRuntimeRequired,
        );
        assert_failure_code(
            dispatch(vec![
                "result".into(),
                "current".into(),
                "--limit".into(),
                "25".into(),
                "--format".into(),
                "json".into(),
            ])
            .await,
            CliErrorCode::GuiRuntimeRequired,
        );

        std::env::remove_var("DBX_APP_DATA_DIR");
    }

    #[tokio::test]
    async fn recognizes_all_eight_cli_commands_with_json_format() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("DBX_APP_DATA_DIR", dir.path());

        let cases = [
            vec!["context", "--format", "json"],
            vec!["conn", "list", "--format", "json"],
            vec!["conn", "show", "__missing__", "--redacted", "--format", "json"],
            vec!["schema", "snapshot", "--format", "json"],
            vec!["safe-query", "--format", "json"],
            vec!["handoff", "--format", "json"],
            vec!["selection", "--format", "json"],
            vec!["result", "current", "--limit", "50", "--format", "json"],
        ];

        for args in cases {
            let env = dispatch(args.iter().map(|value| value.to_string()).collect()).await;
            let json = serde_json::to_value(&env).unwrap();
            assert!(json.get("ok").is_some(), "missing ok for args: {args:?}");
            assert!(json.get("source").is_some(), "missing source for args: {args:?}");
            assert!(
                json.get("data").is_some() || json.get("error").is_some(),
                "missing data/error for args: {args:?}"
            );
        }

        std::env::remove_var("DBX_APP_DATA_DIR");
    }

    #[tokio::test]
    async fn unknown_command_returns_internal_error_envelope() {
        assert_failure_code(
            dispatch(vec!["not-a-command".into(), "--format".into(), "json".into()]).await,
            CliErrorCode::InternalError,
        );
    }

    #[tokio::test]
    async fn rejects_non_json_format() {
        assert_failure_code(
            dispatch(vec!["context".into(), "--format".into(), "text".into()]).await,
            CliErrorCode::InternalError,
        );
    }
}
