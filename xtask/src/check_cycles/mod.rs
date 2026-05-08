use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::io::Error as IoError;
use std::process::{Command, Output};

pub(crate) const PACKAGE: &str = "gitless-sync";
pub(crate) const COMMANDS_PREFIX: &str = "gitless_sync::commands::";

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Graph {
    pub(crate) nodes: BTreeSet<String>,
    pub(crate) edges: BTreeMap<String, BTreeSet<String>>,
}

impl Graph {
    fn empty() -> Self {
        Self {
            nodes: BTreeSet::new(),
            edges: BTreeMap::new(),
        }
    }
}

pub(crate) fn parse_dot(dot: &str) -> Graph {
    let mut graph = Graph::empty();
    for line in dot.lines() {
        if let Some(node) = parse_module_node(line) {
            graph.nodes.insert(node);
        } else if let Some((src, dst)) = parse_uses_edge(line) {
            graph.edges.entry(src).or_default().insert(dst);
        }
    }
    graph
}

fn parse_module_node(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.contains("[label=\"") || !trimmed.contains("\"mod\" node") {
        return None;
    }
    let label_start = trimmed.find("[label=\"")? + "[label=\"".len();
    let label_rest = trimmed.get(label_start..)?;
    let label_end = label_rest.find('"')?;
    let label = &label_rest[..label_end];
    let bar = label.find('|')?;
    let prefix = &label[..bar];
    if prefix != "mod"
        && prefix != "pub mod"
        && prefix != "pub(crate) mod"
        && prefix != "pub(self) mod"
    {
        return None;
    }
    let path = label.get(bar + 1..)?;
    Some(path.to_string())
}

fn parse_uses_edge(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    if !trimmed.contains("[label=\"uses\"") || !trimmed.contains("\"uses\" edge") {
        return None;
    }
    let first_quote = trimmed.find('"')?;
    let after_first = first_quote + 1;
    let rest = trimmed.get(after_first..)?;
    let second_quote = rest.find('"')?;
    let src = &rest[..second_quote];
    let after_second = after_first + second_quote + 1;
    let arrow_rel = trimmed.get(after_second..)?.find("->")?;
    let arrow_pos = after_second + arrow_rel + "->".len();
    let after_arrow = trimmed.get(arrow_pos..)?;
    let third_quote = after_arrow.find('"')?;
    let after_third = arrow_pos + third_quote + 1;
    let last_rest = trimmed.get(after_third..)?;
    let fourth_quote = last_rest.find('"')?;
    let dst = &last_rest[..fourth_quote];
    Some((src.to_string(), dst.to_string()))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Color {
    White,
    Gray,
    Black,
}

fn visit_for_cycles<'a>(
    node: &'a str,
    graph: &'a Graph,
    color: &mut BTreeMap<&'a str, Color>,
    stack: &mut Vec<&'a str>,
    cycles: &mut Vec<Vec<String>>,
) {
    color.insert(node, Color::Gray);
    stack.push(node);
    if let Some(neighbors) = graph.edges.get(node) {
        for next in neighbors {
            if !graph.nodes.contains(next) {
                continue;
            }
            let next_str = next.as_str();
            let next_color = color.get(next_str).copied().unwrap_or(Color::White);
            match next_color {
                Color::Gray => {
                    if let Some(start_idx) = stack.iter().position(|s| *s == next_str) {
                        let mut cycle: Vec<String> =
                            stack[start_idx..].iter().map(ToString::to_string).collect();
                        cycle.push(next_str.to_string());
                        cycles.push(cycle);
                    }
                }
                Color::White => {
                    visit_for_cycles(next_str, graph, color, stack, cycles);
                }
                Color::Black => {}
            }
        }
    }
    stack.pop();
    color.insert(node, Color::Black);
}

pub(crate) fn detect_cycles(graph: &Graph) -> Vec<Vec<String>> {
    let mut color: BTreeMap<&str, Color> = graph
        .nodes
        .iter()
        .map(|n| (n.as_str(), Color::White))
        .collect();
    let mut cycles: Vec<Vec<String>> = Vec::new();

    let nodes: Vec<&str> = graph.nodes.iter().map(String::as_str).collect();
    for node in nodes {
        if color.get(node).copied().unwrap_or(Color::White) == Color::White {
            let mut stack: Vec<&str> = Vec::new();
            visit_for_cycles(node, graph, &mut color, &mut stack, &mut cycles);
        }
    }
    cycles
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CrossSliceViolation {
    pub(crate) source: String,
    pub(crate) target: String,
    pub(crate) source_slice: String,
    pub(crate) target_slice: String,
}

pub(crate) fn detect_cross_slice(graph: &Graph) -> Vec<CrossSliceViolation> {
    let mut out: Vec<CrossSliceViolation> = Vec::new();
    for (src, dsts) in &graph.edges {
        let Some(src_slice) = slice_of(src) else {
            continue;
        };
        for dst in dsts {
            let Some(dst_slice) = slice_of(dst) else {
                continue;
            };
            if src_slice != dst_slice {
                out.push(CrossSliceViolation {
                    source: src.clone(),
                    target: dst.clone(),
                    source_slice: src_slice.to_string(),
                    target_slice: dst_slice.to_string(),
                });
            }
        }
    }
    out
}

fn slice_of(path: &str) -> Option<&str> {
    let rest = path.strip_prefix(COMMANDS_PREFIX)?;
    let end = rest.find("::").unwrap_or(rest.len());
    Some(&rest[..end])
}

pub(crate) fn fetch_dot() -> std::io::Result<String> {
    let args: [&OsStr; 9] = [
        OsStr::new("modules"),
        OsStr::new("dependencies"),
        OsStr::new("--lib"),
        OsStr::new("--package"),
        OsStr::new(PACKAGE),
        OsStr::new("--no-fns"),
        OsStr::new("--no-types"),
        OsStr::new("--no-traits"),
        OsStr::new("--no-sysroot"),
    ];
    let result = Command::new("cargo").args(args).output();
    let Output {
        status,
        stdout,
        stderr,
    } = match result {
        Ok(o) => o,
        Err(err) => {
            return Err(IoError::other(format!(
                "failed to spawn `cargo modules`: {err} (hint: run `cargo install cargo-modules`)"
            )));
        }
    };
    if !status.success() {
        let stderr_str = String::from_utf8_lossy(&stderr);
        return Err(IoError::other(format!(
            "`cargo modules dependencies` exited with {status}: {stderr_str}"
        )));
    }
    String::from_utf8(stdout).map_err(|err| IoError::new(std::io::ErrorKind::InvalidData, err))
}

pub(crate) fn run() -> std::io::Result<u8> {
    let dot = fetch_dot()?;
    let graph = parse_dot(&dot);
    Ok(report(&graph))
}

pub(crate) fn report(graph: &Graph) -> u8 {
    println!(
        "Checking module-level cycles + cross-slice refs in {PACKAGE} ({} modules)",
        graph.nodes.len()
    );

    let cycles = detect_cycles(graph);
    let violations = detect_cross_slice(graph);

    if cycles.is_empty() {
        println!("  cycles:           0");
    } else {
        println!("  cycles:           {} (deny)", cycles.len());
        for cycle in &cycles {
            println!("    cycle: {}", cycle.join(" -> "));
        }
    }

    if violations.is_empty() {
        println!("  cross-slice ref:  0");
    } else {
        println!("  cross-slice ref:  {} (deny)", violations.len());
        for v in &violations {
            println!(
                "    {} ({}) -> {} ({})",
                v.source, v.source_slice, v.target, v.target_slice
            );
        }
    }

    println!();
    if cycles.is_empty() && violations.is_empty() {
        println!("OK: 0 cycles, 0 cross-slice refs.");
        0
    } else {
        println!(
            "FAIL: {} cycle(s), {} cross-slice ref(s).",
            cycles.len(),
            violations.len()
        );
        1
    }
}

#[cfg(test)]
mod tests;
