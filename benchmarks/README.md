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

## Not measured yet

Peak memory, startup of a full engine, CSS parsing, style, layout, paint,
resource loading. They are added as their subsystems land. Compare only against
earlier runs of the same benchmark on the same machine.
