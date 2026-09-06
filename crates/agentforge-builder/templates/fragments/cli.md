## CLI-1. Command-Line Applications

### CLI-1.1 Argument Parsing
- Use a typed argument parser (`clap`) for all user-facing flags; never
  hand-roll `std::env::args` parsing for a public CLI.
- Follow the conventions of the surrounding ecosystem: `--flag value`,
  subcommands over positional overloads, `-h`/`--help` and `--version`
  always present.

### CLI-1.2 Exit Codes & Output
- Distinguish exit codes meaningfully (`0` success, `1` runtime error,
  `2` usage error); document them in `--help`.
- Write diagnostics to `stderr` and machine-readable results to `stdout`.
  Support `--quiet`/`--verbose`; keep default output human-scannable.

### CLI-1.3 Progress & Interactivity
- Only read stdin / prompt when run interactively; detect non-TTY and
  degrade gracefully (no hanging prompts in CI).
- Never print secrets or tokens to logs or error output (§10).

### CLI-1.4 Configuration & Faithfulness
- Load configuration from conventional locations (`~/.config/…`,
  `$XDG_CONFIG_HOME`, project-local) with a clear precedence order
  documented in `--help`.
- `--dry-run` must make zero writes; every mutating flag must be explicit
  (no silent file modification).