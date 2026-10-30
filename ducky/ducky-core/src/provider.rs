use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct Reply {
    pub text: String,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cost_usd: Option<f64>,
}

pub struct Request<'a> {
    pub model: &'a str,
    pub system: &'a str,
    pub user: &'a str,
    pub json_schema: Option<&'a Value>,
}

/// Anything that can turn a prompt into a reply. More backends (direct API, Ollama) slot in here.
pub trait Provider {
    fn complete(&self, req: &Request) -> Result<Reply>;
}

/// Shells out to the headless Claude Code CLI, reusing its login.
pub struct ClaudeCli;

impl Provider for ClaudeCli {
    fn complete(&self, req: &Request) -> Result<Reply> {
        let mut cmd = Command::new("claude");
        cmd.args(["-p", "--output-format", "json", "--no-session-persistence"])
            // Without these, every call carries ~16k tokens of Claude Code tool definitions.
            .args(["--tools", "", "--strict-mcp-config", "--disable-slash-commands"])
            .args(["--model", req.model])
            .args(["--system-prompt", req.system]);
        if let Some(schema) = req.json_schema {
            cmd.args(["--json-schema", &schema.to_string()]);
        }
        cmd.arg(req.user);
        let out = cmd.output().context("failed to run `claude`")?;
        if !out.status.success() {
            bail!("claude exited with {}: {}", out.status, String::from_utf8_lossy(&out.stderr));
        }
        parse_cli_output(&String::from_utf8_lossy(&out.stdout))
    }
}

pub fn parse_cli_output(stdout: &str) -> Result<Reply> {
    let v: Value = serde_json::from_str(stdout.trim()).context("claude output is not JSON")?;
    if v["is_error"].as_bool().unwrap_or(false) {
        bail!("claude reported an error: {}", v["result"]);
    }
    let usage = &v["usage"];
    let n = |k: &str| usage[k].as_u64().unwrap_or(0);
    // Structured output (when --json-schema is used) may arrive as an object instead of a string.
    let text = match (&v["structured_output"], &v["result"]) {
        (s, _) if !s.is_null() => s.to_string(),
        (_, Value::String(s)) => s.clone(),
        (_, other) => other.to_string(),
    };
    Ok(Reply {
        text,
        tokens_in: n("input_tokens") + n("cache_creation_input_tokens") + n("cache_read_input_tokens"),
        tokens_out: n("output_tokens"),
        cost_usd: v["total_cost_usd"].as_f64(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cli_json() {
        let r = parse_cli_output(
            r#"{"is_error":false,"result":"hi","total_cost_usd":0.001,"usage":{"input_tokens":10,"output_tokens":3}}"#,
        )
        .unwrap();
        assert_eq!((r.text.as_str(), r.tokens_in, r.tokens_out), ("hi", 10, 3));
        assert_eq!(r.cost_usd, Some(0.001));
    }

    #[test]
    fn surfaces_errors() {
        assert!(parse_cli_output(r#"{"is_error":true,"result":"boom"}"#).is_err());
    }
}
