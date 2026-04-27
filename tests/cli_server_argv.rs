/// Tests for the `--` sentinel form of trailing launcher args (org-cli-ul8).
///
/// Today: multi-arg launchers must use repeated `--server-arg`. The sentinel
/// form `org --server foo -- --extra=arg <subcommand>` should be an
/// equivalent, less-noisy alternative.
use org_cli::argv::split_sentinel;

const SUBCMDS: &[&str] = &[
    "read",
    "read-headline",
    "outline",
    "query",
    "todo",
    "edit",
    "clock",
    "config",
    "schema",
    "tools",
];

fn s(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| s.to_string()).collect()
}

#[test]
fn test_no_sentinel_returns_unchanged() {
    let argv = s(&["org", "--server", "foo", "tools", "list"]);
    let (cleaned, extra) = split_sentinel(argv.clone(), SUBCMDS);
    assert_eq!(cleaned, argv);
    assert!(extra.is_empty(), "no sentinel ⇒ no extra args");
}

#[test]
fn test_sentinel_extracts_launcher_args() {
    let argv = s(&[
        "org", "--server", "foo", "--", "--bar", "--baz", "tools", "list",
    ]);
    let (cleaned, extra) = split_sentinel(argv, SUBCMDS);
    assert_eq!(
        cleaned,
        s(&["org", "--server", "foo", "tools", "list"]),
        "cleaned argv must drop -- and its trailing args"
    );
    assert_eq!(extra, s(&["--bar", "--baz"]));
}

#[test]
fn test_sentinel_skips_value_of_known_flags() {
    // `--server-arg --` → `--` is the VALUE of --server-arg, not a sentinel.
    let argv = s(&[
        "org",
        "--server",
        "foo",
        "--server-arg",
        "--",
        "tools",
        "list",
    ]);
    let (cleaned, extra) = split_sentinel(argv.clone(), SUBCMDS);
    assert_eq!(cleaned, argv, "value-form `--` must NOT be treated as sentinel");
    assert!(extra.is_empty());
}

#[test]
fn test_sentinel_with_intermixed_compact_flag() {
    let argv = s(&[
        "org",
        "--compact",
        "--server",
        "foo",
        "--",
        "--socket",
        "/tmp/s",
        "tools",
        "list",
    ]);
    let (cleaned, extra) = split_sentinel(argv, SUBCMDS);
    assert_eq!(
        cleaned,
        s(&["org", "--compact", "--server", "foo", "tools", "list"])
    );
    assert_eq!(extra, s(&["--socket", "/tmp/s"]));
}

#[test]
fn test_sentinel_without_subcommand_takes_remainder() {
    // Edge case: `--` with no subcommand following. clap will reject later,
    // but the splitter must not panic and must return the right shape.
    let argv = s(&["org", "--server", "foo", "--", "--bar"]);
    let (cleaned, extra) = split_sentinel(argv, SUBCMDS);
    assert_eq!(cleaned, s(&["org", "--server", "foo"]));
    assert_eq!(extra, s(&["--bar"]));
}

#[test]
fn test_sentinel_handles_timeout_value_flag() {
    // `--timeout 5` is a value flag — its `5` is not the sentinel point.
    let argv = s(&[
        "org",
        "--timeout",
        "5",
        "--server",
        "foo",
        "--",
        "--bar",
        "tools",
        "list",
    ]);
    let (cleaned, extra) = split_sentinel(argv, SUBCMDS);
    assert_eq!(
        cleaned,
        s(&[
            "org", "--timeout", "5", "--server", "foo", "tools", "list"
        ])
    );
    assert_eq!(extra, s(&["--bar"]));
}

// === End-to-end: both --server-arg form and -- sentinel form work against mock ===

fn org_bin() -> std::process::Command {
    std::process::Command::new(env!("CARGO_BIN_EXE_org"))
}

fn mock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mock_org_mcp")
}

/// Both forms must produce identical request logs against the mock:
///   1) org --server <mock> --server-arg --x=y tools list
///   2) org --server <mock> -- --x=y tools list
#[test]
fn test_sentinel_and_server_arg_forms_produce_same_request_log() {
    fn run_form(extra_args: &[&str]) -> Vec<String> {
        let log_path = std::env::temp_dir().join(format!(
            "org_cli_argv_log_{}.jsonl",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ));

        let mut cmd = org_bin();
        cmd.args(["--server", mock_bin()]);
        cmd.args(extra_args);
        cmd.args(["tools", "list"]);
        cmd.env("MOCK_RECORD_REQUESTS", "1");
        cmd.env("MOCK_REQUEST_LOG", log_path.to_str().unwrap());

        let output = cmd.output().expect("failed to run org");
        assert!(
            output.status.success(),
            "form failed: stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );

        let log = std::fs::read_to_string(&log_path).expect("log must exist");
        let _ = std::fs::remove_file(&log_path);
        log.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                let v: serde_json::Value = serde_json::from_str(l).unwrap();
                v["method"].as_str().unwrap_or("").to_string()
            })
            .collect()
    }

    // Mock ignores --extra=foo (it doesn't parse argv). We just need both
    // forms to spawn successfully and complete the protocol.
    let methods_arg_form = run_form(&["--server-arg", "--extra=foo"]);
    let methods_sentinel = run_form(&["--", "--extra=foo"]);

    assert_eq!(
        methods_arg_form, methods_sentinel,
        "both forms must produce identical method sequences"
    );
}
