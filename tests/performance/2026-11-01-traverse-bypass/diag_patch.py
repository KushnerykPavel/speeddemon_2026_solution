"""Adds phase marks and an AnonHugePages report to a main.rs copy (diagnostic builds only)."""
import sys
src, dst = sys.argv[1], sys.argv[2]
t = open(src).read()
def rep(a, b):
    global t
    assert t.count(a) == 1, a
    t = t.replace(a, b)
rep("fn main() -> Result<(), Box<dyn Error>> {\n    tune_memory();",
    "static T0: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();\n"
    "fn mark(l: &str) { eprintln!(\"PHASE {} {:.6}\", l, T0.get().unwrap().elapsed().as_secs_f64()); }\n"
    "fn main() -> Result<(), Box<dyn Error>> {\n    T0.set(std::time::Instant::now()).unwrap();\n    tune_memory();")
rep("    let graph = build_graph(map_file(&data.join(\"graph.txt\"))?, workers)?;",
    "    let graph = build_graph(map_file(&data.join(\"graph.txt\"))?, workers)?;\n    mark(\"load\");")
rep("    let graph = reorder_graph(graph, &mut queries, workers);",
    "    mark(\"queries\");\n    let graph = reorder_graph(graph, &mut queries, workers);\n    mark(\"reorder\");")
rep("    let graphs = [graph, &transpose(graph, workers)];",
    "    let graphs = [graph, &transpose(graph, workers)];\n    mark(\"transpose\");\n"
    "    for l in std::fs::read_to_string(\"/proc/self/smaps_rollup\").unwrap_or_default().lines() { if l.starts_with(\"AnonHugePages\") { eprintln!(\"HUGE {}\", l); } }")
rep("    // Scoped joins make all relaxed answer stores visible before collection.",
    "    mark(\"search\");\n    // Scoped joins make all relaxed answer stores visible before collection.")
rep("    output.flush()?;\n    Ok(())", "    output.flush()?;\n    mark(\"output\");\n    Ok(())")
open(dst, 'w').write(t)
