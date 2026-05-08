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
