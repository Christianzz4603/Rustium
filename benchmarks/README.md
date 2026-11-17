# Benchmarks

`cargo run --release -p parse-bench` parses a synthetic document (nested blocks,
inline formatting, misnested tags, tables, entities, comments, void elements) at
four sizes and prints time per parse, throughput, and node count.

## Baseline (Phase 1, HTML parse only)

Single run in a shared cloud sandbox; treat as order-of-magnitude, not a
reference. Release profile (`lto = "thin"`).

| Input | ms/parse | MB/s | Nodes |
|-------|---------:|-----:|------:|
| 3 KB | 0.065 | 54 | 459 |
| 33 KB | 0.58 | 60 | 4,509 |
| 344 KB | 6.3 | 56 | 45,009 |
| 1.7 MB | 31.5 | 57 | 225,009 |

First parse of a tiny document in a fresh process: about 9 µs.

Throughput is flat across sizes, which is the property to preserve: the first
version of this benchmark showed it falling from 51 to 1.5 MB/s, which exposed a
quadratic scan in comment tokenization (fixed).

## After table modes, in head noscript, script escape states

Same machine class, two runs, release profile. Throughput is still flat across
sizes but about 25% lower than the baseline above. The causes were not profiled;
likely contributors are the extra `tbody`/`tr` nodes and the extra scope checks
and mode dispatch for every table token. Optimizing is deferred until it is
measured with a profiler.

| Input | MB/s (run 1) | MB/s (run 2) | Nodes |
|-------|-------------:|-------------:|------:|
| 33 KB | 41.1 | 43.4 | 4,609 |
| 344 KB | 43.3 | 43.8 | 46,009 |
| 1.7 MB | 43.2 | n/a | 230,009 |

## Profiling and first optimization (instruction counts)

Wall-clock numbers in this sandbox vary by about 25% between runs (39 to 55 MB/s
for the same binary), so optimizations are judged by `valgrind --tool=callgrind`
instruction counts, which are deterministic. Workload: 5 parses of the 200-section
benchmark document (about 68 KB each), release profile with debug info.

Profile before: 108.9M instructions. About 35% was malloc/free. Main sources:
`Atom::new` allocating an `Rc<str>` for every element and attribute name, two
allocations per lowercased name in `lex_tag`, one `String` per text node, and
owned token strings.

| Change | Instructions | vs before |
|--------|-------------:|----------:|
| Before | 108.9M | n/a |
| Static atoms for ~125 common HTML names; single allocation when lowercasing names | 95.9M | -11.9% |

The html5lib pass count was unchanged (1311 / 1891) at that point. This is an instruction count on one
workload, not a throughput claim and not a comparison with any other engine.
Next candidates: zero-copy tokens (borrow from the input instead of owning
`String`s), text stored without a per-node allocation, reserving node capacity.

## After frameset modes, DOCTYPE identifiers, full entity table

Same workload and method as above: 102.5M instructions, up 6.8% from 95.9M. Likely
contributors (not isolated): the per-start-tag `frameset-ok` check, the DOCTYPE state
machine, and binary search over the 2,125-entry entity table for every `&`.
Wall-clock throughput on this run read about 30 MB/s, but the sandbox was also
slower for the same code than in earlier sessions, so wall-clock numbers are not
comparable across sessions; trust only the instruction counts within one method.

## After the select modes, and a first-byte atom lookup

Same workload and method: 94.8M instructions, down from 102.8M (-7.8%) with the select
modes in. Two changes: `Atom::new` now binary-searches only the names that share the
input's first byte (the bucket table is computed at compile time), and `parse_document`
reserves node capacity up front (about one node per 24 bytes of input, capped at 65,536
nodes). Adding 8 more static names (`s`, `tt`, `center`, `track`, `math`, `dir`,
`colspan`, `rowspan`) was worth about 0.1M. That is about 1.2% below the 95.9M reading before
frameset modes, DOCTYPE parsing and the full entity table were added. Still an instruction
count on one workload, not a throughput claim and not a comparison with any other engine.
Biggest remaining costs: malloc/free (about 20%), per-token `String` allocation, and text
node creation.

## Not measured yet

Peak memory, startup of a full engine, CSS parsing, style, layout, paint,
resource loading. They are added as their subsystems land. Compare only against
earlier runs of the same benchmark on the same machine.
