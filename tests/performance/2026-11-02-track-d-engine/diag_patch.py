"""Makes a diagnostic copy: phase marks (wall, process CPU) on stderr, and an expansion count for the ring engine."""
import sys
src, dst = sys.argv[1:]
s = open(src).read()
def rep(a, b, required=True):
    global s
    n = s.count(a)
    assert n == 1 or (not required and n == 0), (a, n)
    s = s.replace(a, b)
rep('fn main() -> Result<(), Box<dyn Error>> {\n    tune_memory();',
    '''static T0: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
static EXPANDED: AtomicU64 = AtomicU64::new(0);
#[repr(C)] struct RU { u: [i64; 2], s: [i64; 2], o: [i64; 14] }
unsafe extern "C" { fn getrusage(who: i32, u: *mut RU) -> i32; }
fn mark(l: &str) { let mut u = RU { u: [0; 2], s: [0; 2], o: [0; 14] }; unsafe { getrusage(0, &mut u) };
    let cpu = (u.u[0] + u.s[0]) as f64 + (u.u[1] + u.s[1]) as f64 * 1e-6;
    eprintln!("PHASE {} {:.6} {:.6}", l, T0.get().unwrap().elapsed().as_secs_f64(), cpu); }
fn main() -> Result<(), Box<dyn Error>> {
    T0.set(std::time::Instant::now()).unwrap();
    tune_memory();''')
rep('    let graph = build_graph(map_file(&data.join("graph.txt"))?, workers)?;\n',
    '    let graph = build_graph(map_file(&data.join("graph.txt"))?, workers)?;\n    mark("load");\n')
rep('    let graph = reorder_graph(graph, &mut queries, workers);\n',
    '    mark("queries");\n    let graph = reorder_graph(graph, &mut queries, workers);\n    mark("reorder");\n')
rep('    let graphs = [graph, &transpose(graph, workers)];\n', '    let graphs = [graph, &transpose(graph, workers)];\n    mark("transpose");\n')
rep('    // Scoped joins make all relaxed answer stores visible before collection.\n',
    '    mark("search");\n    eprintln!("EXPANDED {}", EXPANDED.load(Relaxed));\n    // Scoped joins make all relaxed answer stores visible before collection.\n')
rep('    output.flush()?;\n    Ok(())', '    output.flush()?;\n    mark("output");\n    Ok(())')
rep('        (best != u64::MAX).then_some(best)\n    }\n\n    /// Answers one query with this compact search',
    '        EXPANDED.fetch_add((self.forward.expanded + self.backward.expanded) as u64, Relaxed);\n        (best != u64::MAX).then_some(best)\n    }\n\n    /// Answers one query with this compact search', required=False)
open(dst, 'w').write(s)
