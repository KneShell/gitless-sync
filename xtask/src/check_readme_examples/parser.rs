pub(crate) const QUICK_START_HEADING: &str = "## Quick Start";
pub(crate) const SH_FENCE_OPEN: &str = "```sh";
pub(crate) const SH_FENCE_CLOSE: &str = "```";
pub(crate) const PLACEHOLDER_REPO: &str = "owner/name";
pub(crate) const SUBSTITUTE_REPO: &str = "dummy/dummy";
pub(crate) const BINARY_NAME: &str = "gitless-sync";
pub(crate) const INIT_SUBCOMMAND: &str = "init";

#[derive(Debug, PartialEq, Eq, Clone)]
pub(crate) struct ParsedCommand {
    pub(crate) args: Vec<String>,
    pub(crate) redirect_to: Option<String>,
}

pub(crate) fn extract_quick_start_sh_blocks(readme: &str) -> Vec<String> {
    let mut blocks: Vec<String> = Vec::new();
    let mut in_quick_start = false;
    let mut in_sh_block = false;
    let mut current = String::new();
    for line in readme.lines() {
        if line.starts_with("## ") {
            in_quick_start = line.trim_end() == QUICK_START_HEADING;
            in_sh_block = false;
            current.clear();
            continue;
        }
        if !in_quick_start {
            continue;
        }
        if in_sh_block {
            if line.trim_start().starts_with(SH_FENCE_CLOSE) {
                blocks.push(std::mem::take(&mut current));
                in_sh_block = false;
            } else {
                current.push_str(line);
                current.push('\n');
            }
        } else if line.trim_start().starts_with(SH_FENCE_OPEN) {
            in_sh_block = true;
            current.clear();
        }
    }
    blocks
}

pub(crate) fn extract_executable_lines(block: &str) -> Vec<String> {
    block
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
        .map(ToString::to_string)
        .collect()
}

pub(crate) fn parse_command_line(line: &str) -> ParsedCommand {
    let (cmd_part, redirect_to) = match line.find(" > ") {
        Some(idx) => {
            let cmd = line[..idx].trim();
            let target = line[idx + 3..].trim();
            (cmd, Some(target.to_string()))
        }
        None => (line.trim(), None),
    };
    let substituted = cmd_part.replace(PLACEHOLDER_REPO, SUBSTITUTE_REPO);
    let args: Vec<String> = substituted
        .split_whitespace()
        .map(ToString::to_string)
        .collect();
    ParsedCommand { args, redirect_to }
}

pub(crate) fn is_init_command(parsed: &ParsedCommand) -> bool {
    parsed.args.len() >= 2 && parsed.args[0] == BINARY_NAME && parsed.args[1] == INIT_SUBCOMMAND
}

pub(crate) fn collect_init_commands(readme: &str) -> Vec<ParsedCommand> {
    let blocks = extract_quick_start_sh_blocks(readme);
    let mut out: Vec<ParsedCommand> = Vec::new();
    for block in &blocks {
        for raw in extract_executable_lines(block) {
            let parsed = parse_command_line(&raw);
            if is_init_command(&parsed) {
                out.push(parsed);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_README: &str = "\
# Title

Some intro paragraph.

## Build

```sh
cargo build --release
```

## Quick Start

```sh
# Generate config file once per directory:
gitless-sync init --repo owner/name --branch main > gitless-sync.toml

# Then scan repeatedly without flags:
gitless-sync scan
```

`init` writes nothing on its own.

## Usage

```sh
gitless-sync scan --repo owner/name --local .
```
";

    #[test]
    fn extract_quick_start_returns_only_quick_start_block() {
        let blocks = extract_quick_start_sh_blocks(SAMPLE_README);
        assert_eq!(blocks.len(), 1, "blocks: {blocks:?}");
        assert!(blocks[0].contains("gitless-sync init --repo owner/name --branch main"));
        assert!(
            !blocks[0].contains("cargo build"),
            "Build section sh block leaked"
        );
        assert!(
            !blocks[0].contains("--local"),
            "Usage section sh block leaked"
        );
    }

    #[test]
    fn extract_quick_start_returns_empty_when_section_missing() {
        let readme = "## Build\n\n```sh\ncargo build\n```\n";
        assert!(extract_quick_start_sh_blocks(readme).is_empty());
    }

    #[test]
    fn extract_quick_start_returns_empty_when_section_has_no_sh_fence() {
        let readme = "## Quick Start\n\nProse only, no code block.\n\n## Usage\n";
        assert!(extract_quick_start_sh_blocks(readme).is_empty());
    }

    #[test]
    fn extract_quick_start_handles_multiple_sh_blocks_in_section() {
        let readme = "\
## Quick Start

```sh
gitless-sync init --repo a/b
```

Some prose.

```sh
gitless-sync init --repo c/d
```

## Usage
";
        let blocks = extract_quick_start_sh_blocks(readme);
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn extract_executable_lines_drops_comments_and_blanks() {
        let block = "\
# comment one
gitless-sync init --repo owner/name --branch main > gitless-sync.toml

# comment two
gitless-sync scan
";
        let lines = extract_executable_lines(block);
        assert_eq!(lines.len(), 2);
        assert_eq!(
            lines[0],
            "gitless-sync init --repo owner/name --branch main > gitless-sync.toml"
        );
        assert_eq!(lines[1], "gitless-sync scan");
    }

    #[test]
    fn parse_command_line_splits_redirect_from_args() {
        let parsed = parse_command_line(
            "gitless-sync init --repo owner/name --branch main > gitless-sync.toml",
        );
        assert_eq!(parsed.redirect_to.as_deref(), Some("gitless-sync.toml"));
        assert_eq!(
            parsed.args,
            vec![
                "gitless-sync".to_string(),
                "init".to_string(),
                "--repo".to_string(),
                "dummy/dummy".to_string(),
                "--branch".to_string(),
                "main".to_string(),
            ]
        );
    }

    #[test]
    fn parse_command_line_substitutes_owner_name_placeholder() {
        let parsed = parse_command_line("gitless-sync init --repo owner/name");
        assert!(
            parsed.args.iter().any(|a| a == "dummy/dummy"),
            "owner/name should be substituted to dummy/dummy: {:?}",
            parsed.args
        );
        assert!(
            !parsed.args.iter().any(|a| a == "owner/name"),
            "owner/name placeholder should be replaced: {:?}",
            parsed.args
        );
    }

    #[test]
    fn parse_command_line_no_redirect_when_no_arrow() {
        let parsed = parse_command_line("gitless-sync scan");
        assert!(parsed.redirect_to.is_none());
        assert_eq!(
            parsed.args,
            vec!["gitless-sync".to_string(), "scan".to_string()]
        );
    }

    #[test]
    fn parse_command_line_does_not_split_on_redirect_inside_word() {
        let parsed = parse_command_line("gitless-sync init --repo a/b>c");
        assert!(
            parsed.redirect_to.is_none(),
            "literal `>` inside arg should not be a redirect"
        );
        assert!(parsed.args.iter().any(|a| a.contains('>')));
    }

    #[test]
    fn is_init_command_true_for_init_args() {
        let cmd = parse_command_line("gitless-sync init --repo a/b");
        assert!(is_init_command(&cmd));
    }

    #[test]
    fn is_init_command_false_for_scan_args() {
        let cmd = parse_command_line("gitless-sync scan");
        assert!(!is_init_command(&cmd));
    }

    #[test]
    fn is_init_command_false_for_other_binary() {
        let cmd = parse_command_line("not-our-binary init");
        assert!(!is_init_command(&cmd));
    }

    #[test]
    fn is_init_command_false_for_too_few_args() {
        let cmd = ParsedCommand {
            args: vec!["gitless-sync".to_string()],
            redirect_to: None,
        };
        assert!(!is_init_command(&cmd));
    }

    #[test]
    fn collect_init_commands_picks_only_init_lines_from_quick_start() {
        let commands = collect_init_commands(SAMPLE_README);
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].args[1], "init");
        assert!(commands[0].args.iter().any(|a| a == "dummy/dummy"));
    }
}
