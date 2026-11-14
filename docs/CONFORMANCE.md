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

## Baseline

WPT commit `fa649586f35119af9032312ef21e300e41d1c3b6`, Rustium at the commit that
added the runner:

**1051 of 1891 tests run pass (55.6%); 67 skipped.**

Known causes of the failures, matching the limitations in ARCHITECTURE.md:

| Area | Vectors mostly failing |
|------|------------------------|
| Table insertion modes (no implied `tbody`/`tr`, no foster parenting) | tables01, tests1, tests6, tests9... |
| Foreign content (svg, math) | svg, math, tests10, tests11, tests17, tests24 |
| `<template>` | template.dat |
| In head noscript (scripting off) | noscript01 |
| In select | parts of tests1, tests_innerHTML_1 |
| DOCTYPE identifiers and quirks mode | doctype01, quirks01 |
| Processing-instruction / bogus-comment details | processing-instructions |

Update this file whenever the runner's total changes, with the new WPT commit.
