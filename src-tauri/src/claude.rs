//! Claude connection, subscription mode.
//!
//! Livery AI drives the Driver's own Claude Code CLI as a subprocess. Sign-in
//! happens inside Claude Code; this module never reads, stores, logs or
//! returns a credential. From CLI output it keeps only a version number, an
//! exit code and the documented `authMethod` enum value.
//!
//! CLI contract (code.claude.com/docs/en/cli-reference and /headless):
//! - `claude --version` prints the version number.
//! - `claude auth status` prints JSON and exits 0 when signed in, 1 when not.
//!   `authMethod` is one of `none`, `claude.ai`, `oauth_token`, `api_key`,
//!   `api_key_helper`, `third_party`.
//! - `claude auth login` signs in.
//! - `claude -p <prompt> --output-format json` prints one JSON result and
//!   exits 0 on success.
//!
//! `--bare` is never used: bare mode does not read the subscription sign-in.

use crate::error::{CommandError, CommandResult};
use serde::Serialize;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const VERSION_TIMEOUT: Duration = Duration::from_secs(10);
const AUTH_TIMEOUT: Duration = Duration::from_secs(15);
const TEST_TIMEOUT: Duration = Duration::from_secs(90);
const TEST_PROMPT: &str = "Connection test. Reply with the single word OK.";
/// The command the sign-in terminal runs.
pub const LOGIN_COMMAND: &str = "claude auth login";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SignInState {
    SignedIn,
    SignedOut,
    /// The CLI did not give an answer this app understands.
    Unknown,
}

/// How Claude Code is signed in, from the documented `authMethod` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    /// `claude.ai`: a Claude subscription sign-in.
    Subscription,
    OauthToken,
    ApiKey,
    ApiKeyHelper,
    ThirdParty,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SignIn {
    pub state: SignInState,
    pub method: Option<AuthMethod>,
}

impl SignIn {
    const UNKNOWN: SignIn = SignIn {
        state: SignInState::Unknown,
        method: None,
    };
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClaudeStatus {
    pub cli_found: bool,
    pub version: Option<String>,
    pub sign_in: SignIn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClaudeTest {
    pub ok: bool,
    pub elapsed_ms: u64,
    /// Stable reason code when the test fails: `cli_not_found`, `timeout`,
    /// `spawn_failed`, `exit_code`, `bad_output` or `reported_error`.
    pub reason: Option<&'static str>,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SignInLaunch {
    /// True when a terminal was opened.
    pub launched: bool,
    /// The command the terminal runs, or the Driver can run by hand.
    pub command: &'static str,
}

/// Output of a finished subprocess.
#[derive(Debug)]
pub struct RunOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RunError {
    Spawn,
    Timeout,
}

/// Find the Claude Code CLI on PATH.
pub fn locate_cli() -> Option<PathBuf> {
    // On Windows the native installer ships `claude.exe` and the npm package
    // ships a `claude.cmd` shim. Ask for those by name first so an
    // extension-less shell shim on PATH is never picked.
    #[cfg(windows)]
    const NAMES: [&str; 3] = ["claude.exe", "claude.cmd", "claude"];
    #[cfg(not(windows))]
    const NAMES: [&str; 1] = ["claude"];
    NAMES.iter().find_map(|name| which::which(name).ok())
}

fn base_command(program: &std::path::Path) -> Command {
    let mut cmd = Command::new(program);
    cmd.stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console flashes up behind the app.
        cmd.creation_flags(0x0800_0000);
    }
    cmd
}

/// Run a command to completion, killing it after `timeout`. Only stdout is
/// kept; stderr is discarded so it can never be logged or returned.
pub fn run_with_timeout(mut cmd: Command, timeout: Duration) -> Result<RunOutput, RunError> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::null());
    let mut child = cmd.spawn().map_err(|_| RunError::Spawn)?;
    let mut pipe = child.stdout.take().ok_or(RunError::Spawn)?;
    // Drain stdout on its own thread so a chatty child cannot block on a
    // full pipe while this thread waits for it to exit.
    let reader = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = pipe.read_to_end(&mut buf);
        buf
    });

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(RunError::Timeout);
            }
            Ok(None) => thread::sleep(Duration::from_millis(25)),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(RunError::Spawn);
            }
        }
    };
    let stdout = reader.join().unwrap_or_default();
    Ok(RunOutput {
        exit_code: status.code(),
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
    })
}

/// Pull the version number out of `claude --version` output, such as
/// `2.1.325 (Claude Code)`. Returns nothing that is not a dotted number.
pub fn parse_version(output: &str) -> Option<String> {
    output
        .split_whitespace()
        .map(|token| token.trim_start_matches('v'))
        .find(|token| {
            let core = token.split(['-', '+']).next().unwrap_or(token);
            let parts: Vec<&str> = core.split('.').collect();
            (2..=4).contains(&parts.len())
                && parts
                    .iter()
                    .all(|p| !p.is_empty() && p.len() <= 6 && p.bytes().all(|b| b.is_ascii_digit()))
                && token.len() <= 32
                && token
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'+'))
        })
        .map(str::to_string)
}

/// Interpret `claude auth status`. The exit code decides signed in or out and
/// must agree with the JSON; anything else is `Unknown`. Only the
/// `authMethod` enum value is read from the JSON. Every other field (account,
/// organisation, paths) is ignored.
pub fn parse_auth_status(exit_code: Option<i32>, stdout: &str) -> SignIn {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(stdout.trim()) else {
        return SignIn::UNKNOWN;
    };
    let method = match json.get("authMethod").and_then(|m| m.as_str()) {
        Some("none") => None,
        Some("claude.ai") => Some(AuthMethod::Subscription),
        Some("oauth_token") => Some(AuthMethod::OauthToken),
        Some("api_key") => Some(AuthMethod::ApiKey),
        Some("api_key_helper") => Some(AuthMethod::ApiKeyHelper),
        Some("third_party") => Some(AuthMethod::ThirdParty),
        _ => return SignIn::UNKNOWN,
    };
    match (exit_code, method) {
        (Some(0), Some(method)) => SignIn {
            state: SignInState::SignedIn,
            method: Some(method),
        },
        (Some(1), None) => SignIn {
            state: SignInState::SignedOut,
            method: None,
        },
        _ => SignIn::UNKNOWN,
    }
}

/// Interpret a `claude -p --output-format json` run.
pub fn parse_test_result(
    exit_code: Option<i32>,
    stdout: &str,
) -> Result<(), (&'static str, Option<i32>)> {
    let json = serde_json::from_str::<serde_json::Value>(stdout.trim()).ok();
    let reported_error = json
        .as_ref()
        .and_then(|j| j.get("is_error"))
        .and_then(|e| e.as_bool())
        .unwrap_or(false);
    if reported_error {
        return Err(("reported_error", exit_code));
    }
    if exit_code != Some(0) {
        return Err(("exit_code", exit_code));
    }
    match json {
        Some(j) if j.get("result").is_some_and(|r| r.is_string()) => Ok(()),
        _ => Err(("bad_output", exit_code)),
    }
}

/// CLI presence, version and sign-in state.
pub fn status() -> ClaudeStatus {
    let Some(cli) = locate_cli() else {
        return ClaudeStatus {
            cli_found: false,
            version: None,
            sign_in: SignIn::UNKNOWN,
        };
    };

    let mut version_cmd = base_command(&cli);
    version_cmd.arg("--version");
    let version = run_with_timeout(version_cmd, VERSION_TIMEOUT)
        .ok()
        .filter(|out| out.exit_code == Some(0))
        .and_then(|out| parse_version(&out.stdout));

    let mut auth_cmd = base_command(&cli);
    auth_cmd.args(["auth", "status"]);
    let sign_in = match run_with_timeout(auth_cmd, AUTH_TIMEOUT) {
        Ok(out) => parse_auth_status(out.exit_code, &out.stdout),
        Err(_) => SignIn::UNKNOWN,
    };

    ClaudeStatus {
        cli_found: true,
        version,
        sign_in,
    }
}

/// Run one tiny non-interactive prompt through the CLI with the Driver's
/// sign-in and report pass or fail with the elapsed time.
pub fn test(model: &str) -> ClaudeTest {
    let started = Instant::now();
    let fail = |reason, exit_code, started: Instant| ClaudeTest {
        ok: false,
        elapsed_ms: started.elapsed().as_millis() as u64,
        reason: Some(reason),
        exit_code,
    };
    let Some(cli) = locate_cli() else {
        return fail("cli_not_found", None, started);
    };
    let mut cmd = base_command(&cli);
    cmd.args([
        "-p",
        TEST_PROMPT,
        "--output-format",
        "json",
        "--model",
        model,
    ]);
    // Run from a neutral folder so no project's CLAUDE.md, hooks or MCP
    // servers are picked up by the test.
    cmd.current_dir(std::env::temp_dir());
    match run_with_timeout(cmd, TEST_TIMEOUT) {
        Err(RunError::Timeout) => fail("timeout", None, started),
        Err(RunError::Spawn) => fail("spawn_failed", None, started),
        Ok(out) => match parse_test_result(out.exit_code, &out.stdout) {
            Ok(()) => ClaudeTest {
                ok: true,
                elapsed_ms: started.elapsed().as_millis() as u64,
                reason: None,
                exit_code: out.exit_code,
            },
            Err((reason, code)) => fail(reason, code, started),
        },
    }
}

/// Open a terminal running Claude Code's own sign-in command. The sign-in
/// itself happens in Claude Code and the browser, never in this app.
pub fn open_signin() -> CommandResult<SignInLaunch> {
    if locate_cli().is_none() {
        return Err(CommandError::not_found(
            "Claude Code was not found on PATH. Install it, then sign in.",
        ));
    }
    spawn_signin_terminal()?;
    Ok(SignInLaunch {
        launched: true,
        command: LOGIN_COMMAND,
    })
}

#[cfg(windows)]
fn spawn_signin_terminal() -> CommandResult<()> {
    use std::os::windows::process::CommandExt;
    // `start` opens a new console window; `cmd /k` keeps it open so the
    // Driver can read the result. The first quoted argument is the title.
    Command::new("cmd")
        .raw_arg(r#"/c start "Claude sign-in" cmd /k claude auth login"#)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(0x0800_0000)
        .spawn()
        .map(|_| ())
        .map_err(|e| CommandError::io("Open sign-in terminal", &e))
}

#[cfg(not(windows))]
fn spawn_signin_terminal() -> CommandResult<()> {
    // Development fallback for Linux desktops. Livery AI ships for Windows.
    let terminals: [(&str, &[&str]); 4] = [
        ("x-terminal-emulator", &["-e"]),
        ("gnome-terminal", &["--"]),
        ("konsole", &["-e"]),
        ("xterm", &["-e"]),
    ];
    for (terminal, prefix) in terminals {
        if which::which(terminal).is_err() {
            continue;
        }
        let spawned = Command::new(terminal)
            .args(prefix)
            .args(["claude", "auth", "login"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        if spawned.is_ok() {
            return Ok(());
        }
    }
    Err(CommandError::new(
        "no_terminal",
        format!("No terminal was found. Run `{LOGIN_COMMAND}` in a terminal."),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_version_output() {
        assert_eq!(
            parse_version("2.1.325 (Claude Code)\n").as_deref(),
            Some("2.1.325")
        );
        assert_eq!(parse_version("v2.0.0").as_deref(), Some("2.0.0"));
        assert_eq!(
            parse_version("claude 3.4.5-beta.1").as_deref(),
            Some("3.4.5-beta.1")
        );
        assert_eq!(parse_version(""), None);
        assert_eq!(parse_version("command not found"), None);
        assert_eq!(parse_version("sk-ant-1.2.3"), None);
        assert_eq!(parse_version("1234567.1.1"), None);
    }

    #[test]
    fn auth_status_follows_the_documented_contract() {
        let signed_in = |method| SignIn {
            state: SignInState::SignedIn,
            method: Some(method),
        };
        assert_eq!(
            parse_auth_status(Some(0), r#"{"loggedIn":true,"authMethod":"claude.ai"}"#),
            signed_in(AuthMethod::Subscription)
        );
        assert_eq!(
            parse_auth_status(Some(0), r#"{"authMethod":"api_key"}"#),
            signed_in(AuthMethod::ApiKey)
        );
        assert_eq!(
            parse_auth_status(Some(0), r#"{"authMethod":"oauth_token"}"#),
            signed_in(AuthMethod::OauthToken)
        );
        assert_eq!(
            parse_auth_status(Some(0), r#"{"authMethod":"api_key_helper"}"#),
            signed_in(AuthMethod::ApiKeyHelper)
        );
        assert_eq!(
            parse_auth_status(Some(0), r#"{"authMethod":"third_party"}"#),
            signed_in(AuthMethod::ThirdParty)
        );
        assert_eq!(
            parse_auth_status(Some(1), "{\"authMethod\":\"none\"}\n"),
            SignIn {
                state: SignInState::SignedOut,
                method: None
            }
        );
    }

    #[test]
    fn auth_status_is_unknown_when_the_answer_is_unclear() {
        let cases = [
            (Some(0), "Logged in"),
            (Some(1), "error: unknown command 'auth'"),
            (Some(0), ""),
            (Some(0), "{}"),
            (Some(0), r#"{"authMethod":"something_new"}"#),
            (Some(0), r#"{"authMethod":"none"}"#),
            (Some(1), r#"{"authMethod":"claude.ai"}"#),
            (Some(2), r#"{"authMethod":"claude.ai"}"#),
            (None, r#"{"authMethod":"claude.ai"}"#),
        ];
        for (code, out) in cases {
            assert_eq!(parse_auth_status(code, out), SignIn::UNKNOWN, "{out:?}");
        }
    }

    #[test]
    fn auth_status_never_carries_account_fields() {
        let sign_in = parse_auth_status(
            Some(0),
            r#"{"authMethod":"claude.ai","email":"driver@example.com","orgId":"org-1"}"#,
        );
        let json = serde_json::to_string(&sign_in).unwrap();
        assert_eq!(json, r#"{"state":"signed_in","method":"subscription"}"#);
    }

    #[test]
    fn test_result_parsing() {
        assert_eq!(
            parse_test_result(
                Some(0),
                r#"{"type":"result","is_error":false,"result":"OK"}"#
            ),
            Ok(())
        );
        assert_eq!(parse_test_result(Some(0), r#"{"result":"OK"}"#), Ok(()));
        assert_eq!(
            parse_test_result(Some(1), r#"{"is_error":true,"result":"Not logged in"}"#),
            Err(("reported_error", Some(1)))
        );
        assert_eq!(
            parse_test_result(Some(0), r#"{"is_error":true,"result":"x"}"#),
            Err(("reported_error", Some(0)))
        );
        assert_eq!(
            parse_test_result(Some(1), "boom"),
            Err(("exit_code", Some(1)))
        );
        assert_eq!(parse_test_result(None, ""), Err(("exit_code", None)));
        assert_eq!(
            parse_test_result(Some(0), "OK"),
            Err(("bad_output", Some(0)))
        );
        assert_eq!(
            parse_test_result(Some(0), "{}"),
            Err(("bad_output", Some(0)))
        );
    }

    #[cfg(unix)]
    mod process {
        use super::*;

        fn sh(script: &str) -> Command {
            let mut cmd = Command::new("sh");
            cmd.args(["-c", script]).stdin(Stdio::null());
            cmd
        }

        #[test]
        fn captures_stdout_and_exit_code_but_not_stderr() {
            let out = run_with_timeout(
                sh("echo visible; echo hidden 1>&2; exit 3"),
                Duration::from_secs(10),
            )
            .unwrap();
            assert_eq!(out.exit_code, Some(3));
            assert_eq!(out.stdout, "visible\n");
        }

        #[test]
        fn kills_a_process_that_overruns() {
            let started = Instant::now();
            let err = run_with_timeout(sh("exec sleep 30"), Duration::from_millis(200));
            assert_eq!(err.unwrap_err(), RunError::Timeout);
            assert!(started.elapsed() < Duration::from_secs(5));
        }

        #[test]
        fn handles_output_larger_than_a_pipe_buffer() {
            let out = run_with_timeout(
                sh("head -c 300000 /dev/zero | tr '\\0' 'a'"),
                Duration::from_secs(10),
            )
            .unwrap();
            assert_eq!(out.stdout.len(), 300_000);
        }

        #[test]
        fn missing_program_is_a_spawn_error() {
            let cmd = Command::new("/nonexistent/liverai-test-binary");
            assert_eq!(
                run_with_timeout(cmd, Duration::from_secs(1)).unwrap_err(),
                RunError::Spawn
            );
        }
    }
}
