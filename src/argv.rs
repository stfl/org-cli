/// argv preprocessing for the `--` sentinel form of trailing launcher args.
///
/// clap-derive does not naturally support "everything between `--` and the
/// first subcommand belongs to a Vec<String> argument", because clap's own
/// `--` handling would consume those tokens before the subcommand router
/// sees them. We do the split ourselves before calling `Cli::parse_from`.
///
/// See `tests/cli_server_argv.rs` for the contract.
///
/// Splits argv at the first top-level `--` token, routing the tokens between
/// `--` and the first known subcommand into a separate `extra` vector. Tokens
/// that appear as the value of a known value-flag (`--server`, `--server-arg`,
/// `--timeout`) are skipped during the search so e.g. `--server-arg --` is
/// treated as a literal value, not as a sentinel.
///
/// # Returns
/// `(cleaned, extra)` where `cleaned` is suitable for `Cli::parse_from` and
/// `extra` should be appended to `cli.server_args`.
pub fn split_sentinel(argv: Vec<String>, subcommands: &[&str]) -> (Vec<String>, Vec<String>) {
    let value_flags: &[&str] = &["--server", "--server-arg", "--timeout"];

    let mut i = if argv.is_empty() { 0 } else { 1 };
    let mut sentinel_idx: Option<usize> = None;
    while i < argv.len() {
        let tok = argv[i].as_str();
        if tok == "--" {
            sentinel_idx = Some(i);
            break;
        }
        if value_flags.contains(&tok) {
            i += 2;
        } else {
            i += 1;
        }
    }

    let Some(sentinel) = sentinel_idx else {
        return (argv, Vec::new());
    };

    let next_sub = (sentinel + 1..argv.len()).find(|&j| subcommands.contains(&argv[j].as_str()));

    let mut cleaned: Vec<String> = argv[..sentinel].to_vec();
    let extra: Vec<String> = match next_sub {
        Some(j) => {
            cleaned.extend_from_slice(&argv[j..]);
            argv[sentinel + 1..j].to_vec()
        }
        None => argv[sentinel + 1..].to_vec(),
    };
    (cleaned, extra)
}
