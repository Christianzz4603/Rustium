# HTML conformance status

Measured, not claimed. Rustium does **not** claim HTML standards compliance;
this file records what the tree-construction vectors currently show.

## How to reproduce

The vectors are maintained in web-platform-tests (html5lib-tests moved its
tree-construction data there) and are not vendored here.

```sh
git clone --depth 1 --filter=blob:none --sparse https://github.com/web-platform-tests/wpt
git -C wpt sparse-checkout set html/syntax/parsing
cargo run -p html5lib-runner -- wpt/html/syntax/parsing/resources            # summary
cargo run -p html5lib-runner -- wpt/html/syntax/parsing/resources -v --file tests1.dat --limit 5
```

Run in the debug profile: release builds use `panic = "abort"`, and the runner
relies on catching panics so one crash counts as one failure.

## What is compared

- Only the resulting tree, dumped in the html5lib format. The `#errors` lists are **not** compared.
- Tests marked `#script-off` run with `scripting: false`; all others with `scripting: true`.
- **Skipped** (counted separately, never as passes): fragment tests whose context element is namespaced (`svg x`, `math x`).
- A panic is a failure.

## Results

WPT commit `fa649586f35119af9032312ef21e300e41d1c3b6`.

| Stage | Pass / run | Rate | Skipped |
|-------|-----------:|-----:|--------:|
| Runner introduced (simplified tables) | 1051 / 1891 | 55.6% | 67 |
| Table mode family + foster parenting + quirks approximation | 1221 / 1891 | 64.6% | 67 |
| In head noscript | 1247 / 1891 | 65.9% | 67 |
| Script-data escape states, `</script` at EOF | 1311 / 1891 | 69.3% | 67 |
| Frameset modes + `frameset-ok` | 1376 / 1891 | 72.8% | 67 |

Files where every run vector passes: `blocks`, `comments01`, `inbody01`, `isindex`,
`menuitem-element`, `noscript01`, `ruby`, `scriptdata01`, `tests3`, `tests5`, `tests8`,
`tests14`, `tests15`, `tests16`, `tests17`, `tests22`, `tests23`, `tests25`, `tricky01`,
`void-in-phrasing`, `pending-spec-changes-plain-text-unsafe`.

## Remaining failures, by cause

| Area | Vectors (fail count) |
|------|----------------------|
| `<template>` (no template contents, no template insertion mode) | template.dat (125) |
| Processing-instruction nodes expected by current vectors; Rustium emits bogus comments | processing-instructions.dat (84) |
| Foreign content (svg, math, namespaced attributes, integration points) | tests9 (27), tests10 (54), tests11, tests19 (part), tests24, svg, math |
| DOCTYPE public/system identifiers and exact quirks mode | doctype01 (16), quirks01, parts of others |
| In select | parts of tests1, tests2, webkit, tests_innerHTML_1 |
| `tests21`, `domjs-unsafe`, most of `plain-text-unsafe`, `webkit02`, `tests20`, `tests26` | triaged by sampling: SVG/MathML foreign content (including `<![CDATA[`), so they are covered by the foreign-content row |

Triage was done by reading the first failures of each file, not every failure; some failures in these files may have other causes.

Update this file whenever the runner's total changes, with the WPT commit used.
