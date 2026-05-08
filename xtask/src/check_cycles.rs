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

pub(crate) fn detect_cycles(graph: &Graph) -> Vec<Vec<String>> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Color {
        White,
        Gray,
        Black,
    }

    let mut color: BTreeMap<&str, Color> = graph
        .nodes
        .iter()
        .map(|n| (n.as_str(), Color::White))
        .collect();
    let mut cycles: Vec<Vec<String>> = Vec::new();

    fn visit<'a>(
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
                                stack[start_idx..].iter().map(|s| s.to_string()).collect();
                            cycle.push(next_str.to_string());
                            cycles.push(cycle);
                        }
                    }
                    Color::White => {
                        visit(next_str, graph, color, stack, cycles);
                    }
                    Color::Black => {}
                }
            }
        }
        stack.pop();
        color.insert(node, Color::Black);
    }

    let nodes: Vec<&str> = graph.nodes.iter().map(String::as_str).collect();
    for node in nodes {
        if color.get(node).copied().unwrap_or(Color::White) == Color::White {
            let mut stack: Vec<&str> = Vec::new();
            visit(node, graph, &mut color, &mut stack, &mut cycles);
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
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    const TWO_NODES_NO_EDGE: &str = r##"
    "gitless_sync::commands::scan" [label="pub mod|gitless_sync::commands::scan", fillcolor="#81c169"]; // "mod" node
    "gitless_sync::shared::error" [label="pub mod|gitless_sync::shared::error", fillcolor="#81c169"]; // "mod" node
"##;

    #[test]
    fn parse_dot_collects_module_nodes() {
        let g = parse_dot(TWO_NODES_NO_EDGE);
        assert_eq!(g.nodes.len(), 2);
        assert!(g.nodes.contains("gitless_sync::commands::scan"));
        assert!(g.nodes.contains("gitless_sync::shared::error"));
        assert!(g.edges.is_empty());
    }

    #[test]
    fn parse_dot_skips_non_mod_nodes() {
        let dot = r##"
    "gitless_sync::shared::error::GitlessError" [label="enum|gitless_sync::shared::error::GitlessError", fillcolor="#000"]; // "enum" node
    "gitless_sync::shared::error" [label="pub mod|gitless_sync::shared::error", fillcolor="#81c169"]; // "mod" node
"##;
        let g = parse_dot(dot);
        assert_eq!(g.nodes.len(), 1);
        assert!(g.nodes.contains("gitless_sync::shared::error"));
    }

    #[test]
    fn parse_dot_collects_uses_edges() {
        let dot = r##"
    "gitless_sync::commands::scan" [label="pub mod|gitless_sync::commands::scan", fillcolor="#81c169"]; // "mod" node
    "gitless_sync::shared::error" [label="pub mod|gitless_sync::shared::error", fillcolor="#81c169"]; // "mod" node
    "gitless_sync::commands::scan" -> "gitless_sync::shared::error" [label="uses", color="#7f7f7f", style="dashed"] [constraint=false]; // "uses" edge
"##;
        let g = parse_dot(dot);
        let scan_edges = g.edges.get("gitless_sync::commands::scan").unwrap();
        assert!(scan_edges.contains("gitless_sync::shared::error"));
    }

    #[test]
    fn parse_dot_accepts_pub_crate_and_pub_self_mods() {
        let dot = r##"
    "a::b" [label="pub(crate) mod|a::b", fillcolor="#81c169"]; // "mod" node
    "a::c" [label="pub(self) mod|a::c", fillcolor="#81c169"]; // "mod" node
    "a::d" [label="mod|a::d", fillcolor="#81c169"]; // "mod" node
"##;
        let g = parse_dot(dot);
        assert_eq!(g.nodes.len(), 3);
        assert!(g.nodes.contains("a::b"));
        assert!(g.nodes.contains("a::c"));
        assert!(g.nodes.contains("a::d"));
    }

    #[test]
    fn parse_dot_skips_unknown_prefix() {
        let dot = r##"
    "a::b" [label="weird prefix|a::b", fillcolor="#81c169"]; // "mod" node
"##;
        let g = parse_dot(dot);
        assert!(g.nodes.is_empty());
    }

    #[test]
    fn parse_dot_skips_owns_edges() {
        let dot = r##"
    "gitless_sync::commands" [label="pub mod|gitless_sync::commands", fillcolor="#81c169"]; // "mod" node
    "gitless_sync::commands::scan" [label="pub mod|gitless_sync::commands::scan", fillcolor="#81c169"]; // "mod" node
    "gitless_sync::commands" -> "gitless_sync::commands::scan" [label="owns", color="#000000", style="solid"] [constraint=true]; // "owns" edge
"##;
        let g = parse_dot(dot);
        assert!(g.edges.is_empty());
    }

    #[test]
    fn detect_cycles_empty_graph_returns_empty() {
        let g = Graph::empty();
        let cycles = detect_cycles(&g);
        assert!(cycles.is_empty());
    }

    #[test]
    fn detect_cycles_acyclic_graph_returns_empty() {
        let mut g = Graph::empty();
        g.nodes.insert("a".into());
        g.nodes.insert("b".into());
        g.nodes.insert("c".into());
        g.edges.entry("a".into()).or_default().insert("b".into());
        g.edges.entry("b".into()).or_default().insert("c".into());
        let cycles = detect_cycles(&g);
        assert!(cycles.is_empty());
    }

    #[test]
    fn detect_cycles_two_node_cycle_detected() {
        let mut g = Graph::empty();
        g.nodes.insert("a".into());
        g.nodes.insert("b".into());
        g.edges.entry("a".into()).or_default().insert("b".into());
        g.edges.entry("b".into()).or_default().insert("a".into());
        let cycles = detect_cycles(&g);
        assert_eq!(cycles.len(), 1);
        let c = &cycles[0];
        assert!(c.contains(&"a".to_string()));
        assert!(c.contains(&"b".to_string()));
    }

    #[test]
    fn detect_cycles_self_loop_detected() {
        let mut g = Graph::empty();
        g.nodes.insert("a".into());
        g.edges.entry("a".into()).or_default().insert("a".into());
        let cycles = detect_cycles(&g);
        assert_eq!(cycles.len(), 1);
    }

    #[test]
    fn detect_cycles_three_node_cycle_detected() {
        let mut g = Graph::empty();
        for n in ["a", "b", "c"] {
            g.nodes.insert(n.into());
        }
        g.edges.entry("a".into()).or_default().insert("b".into());
        g.edges.entry("b".into()).or_default().insert("c".into());
        g.edges.entry("c".into()).or_default().insert("a".into());
        let cycles = detect_cycles(&g);
        assert_eq!(cycles.len(), 1);
    }

    #[test]
    fn detect_cycles_ignores_edges_to_unknown_nodes() {
        let mut g = Graph::empty();
        g.nodes.insert("a".into());
        g.edges
            .entry("a".into())
            .or_default()
            .insert("ghost".into());
        let cycles = detect_cycles(&g);
        assert!(cycles.is_empty());
    }

    #[test]
    fn detect_cross_slice_no_violations_for_shared_targets() {
        let mut g = Graph::empty();
        g.nodes.insert("gitless_sync::commands::scan".into());
        g.nodes.insert("gitless_sync::shared::error".into());
        g.edges
            .entry("gitless_sync::commands::scan".into())
            .or_default()
            .insert("gitless_sync::shared::error".into());
        let v = detect_cross_slice(&g);
        assert!(v.is_empty());
    }

    #[test]
    fn detect_cross_slice_no_violations_for_same_slice_descents() {
        let mut g = Graph::empty();
        g.nodes.insert("gitless_sync::commands::scan".into());
        g.nodes
            .insert("gitless_sync::commands::scan::compare".into());
        g.edges
            .entry("gitless_sync::commands::scan".into())
            .or_default()
            .insert("gitless_sync::commands::scan::compare".into());
        let v = detect_cross_slice(&g);
        assert!(v.is_empty());
    }

    #[test]
    fn detect_cross_slice_flags_scan_to_diff() {
        let mut g = Graph::empty();
        g.nodes.insert("gitless_sync::commands::scan".into());
        g.nodes.insert("gitless_sync::commands::diff".into());
        g.edges
            .entry("gitless_sync::commands::scan".into())
            .or_default()
            .insert("gitless_sync::commands::diff".into());
        let v = detect_cross_slice(&g);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].source_slice, "scan");
        assert_eq!(v[0].target_slice, "diff");
    }

    #[test]
    fn detect_cross_slice_flags_nested_diff_to_scan() {
        let mut g = Graph::empty();
        g.nodes.insert("gitless_sync::commands::diff".into());
        g.nodes
            .insert("gitless_sync::commands::scan::compare".into());
        g.edges
            .entry("gitless_sync::commands::diff".into())
            .or_default()
            .insert("gitless_sync::commands::scan::compare".into());
        let v = detect_cross_slice(&g);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].source_slice, "diff");
        assert_eq!(v[0].target_slice, "scan");
    }

    #[test]
    fn slice_of_returns_top_segment() {
        assert_eq!(slice_of("gitless_sync::commands::scan"), Some("scan"));
        assert_eq!(
            slice_of("gitless_sync::commands::scan::graphql"),
            Some("scan")
        );
        assert_eq!(slice_of("gitless_sync::shared::error"), None);
        assert_eq!(slice_of("other::path"), None);
    }

    #[test]
    fn report_returns_zero_for_clean_graph() {
        let mut g = Graph::empty();
        g.nodes.insert("gitless_sync::commands::scan".into());
        g.nodes.insert("gitless_sync::shared::error".into());
        g.edges
            .entry("gitless_sync::commands::scan".into())
            .or_default()
            .insert("gitless_sync::shared::error".into());
        assert_eq!(report(&g), 0);
    }

    #[test]
    fn report_returns_one_when_cycle_present() {
        let mut g = Graph::empty();
        g.nodes.insert("a".into());
        g.nodes.insert("b".into());
        g.edges.entry("a".into()).or_default().insert("b".into());
        g.edges.entry("b".into()).or_default().insert("a".into());
        assert_eq!(report(&g), 1);
    }

    #[test]
    fn report_returns_one_when_cross_slice_violation_present() {
        let mut g = Graph::empty();
        g.nodes.insert("gitless_sync::commands::scan".into());
        g.nodes.insert("gitless_sync::commands::diff".into());
        g.edges
            .entry("gitless_sync::commands::scan".into())
            .or_default()
            .insert("gitless_sync::commands::diff".into());
        assert_eq!(report(&g), 1);
    }

    const REAL_GRAPH_FIXTURE: &str = r##"digraph {
    "gitless_sync::commands" [label="pub mod|gitless_sync::commands", fillcolor="#81c169"]; // "mod" node
    "gitless_sync::commands::diff" [label="pub mod|gitless_sync::commands::diff", fillcolor="#81c169"]; // "mod" node
    "gitless_sync::commands::init" [label="pub mod|gitless_sync::commands::init", fillcolor="#81c169"]; // "mod" node
    "gitless_sync::commands::scan" [label="pub mod|gitless_sync::commands::scan", fillcolor="#81c169"]; // "mod" node
    "gitless_sync::commands::scan::compare" [label="pub mod|gitless_sync::commands::scan::compare", fillcolor="#81c169"]; // "mod" node
    "gitless_sync::shared" [label="pub mod|gitless_sync::shared", fillcolor="#81c169"]; // "mod" node
    "gitless_sync::shared::error" [label="pub mod|gitless_sync::shared::error", fillcolor="#81c169"]; // "mod" node
    "gitless_sync::commands::diff" -> "gitless_sync::shared::error" [label="uses", color="#7f7f7f", style="dashed"] [constraint=false]; // "uses" edge
    "gitless_sync::commands::scan" -> "gitless_sync::commands::scan::compare" [label="uses", color="#7f7f7f", style="dashed"] [constraint=false]; // "uses" edge
    "gitless_sync::commands::scan" -> "gitless_sync::shared::error" [label="uses", color="#7f7f7f", style="dashed"] [constraint=false]; // "uses" edge
}"##;

    #[test]
    fn parse_dot_real_graph_fixture_no_cycles_no_violations() {
        let g = parse_dot(REAL_GRAPH_FIXTURE);
        assert_eq!(g.nodes.len(), 7);
        assert!(detect_cycles(&g).is_empty());
        assert!(detect_cross_slice(&g).is_empty());
    }
}
