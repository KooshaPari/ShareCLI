# ShareCLI / Fino Fabric audit and dream-loop review pack

Read these in order:

1. [Technical audit](sharecli-audit.md): current local implementation, eight prioritized findings, native Windows/WSL gaps, evidence limits and remediation sequence.
2. [Console and optimization design](console-optimization-design.md): proposed always-useful console, general-process and unknown-harness behavior, rules, learned optimizations and attributable savings.
3. [PhenoRegistry dream-loop plan](dream-loop-phenoregistry.md): proposed canonical planning artifact, product boundaries, staged delivery and acceptance evidence.

## Supporting evidence

- [Repository inventory](inventory.md): exact checkout identity, dirty-state preservation, component/test inventory and tool limitations.
- [Source register](source-register.md): recovered conversation and external-source provenance, inaccessible sources and remaining unknowns.
- [Exact operator brief](operator-brief.md): requested wording and Windows-native addendum.
- [Selected chat excerpts](selected-chat-excerpts.md): readable recovered context; consult the source register for recovery completeness.
- `chat-export-inventory.json` and `chat-*.json`: supporting local chat exports, where available.

PDF and ZIP review bundles are produced by the packaging lane after all primary reports are finalized; see the generated package manifest/checksums for exact contents. JSON exports may contain personal conversation material and should be reviewed before sharing.

## Scope and acceptance limits

This is a read-only audit and proposed design. Source snapshot: ShareCLI `fec57c5ce45935203c63e5b4c487fddf642ff18c`, observed 2026-10-14 Pacific. Existing source changes and processes were preserved. No screenshots or recordings were made. No implementation, runtime deployment, benchmark result, numeric savings, Windows runtime acceptance, or PhenoRegistry write is implied by a planning document. Current GitHub PR/issue/security-alert state was unavailable in the technical lane because network queries failed.

The most urgent source findings are missing native Windows spawn/wait functionality, a referenced dashboard script with no matching inspected HTTP route, and invalid savings attribution in that script. Runtime impact remains to be confirmed against the actual installed web service using text-only observation.

## Downloadable artifacts

- [Combined text PDF](pdf/sharecli-comprehensive-report.pdf): audit, console design and dream-loop plan.
- [Report bundle](sharecli-report-bundle.zip): reports, index, source register, exact brief and PDF; embedded SHA256 manifest.
- [Private selected-chat export bundle](sharecli-selected-chat-exports.zip): five recovered conversation snapshots plus selected excerpts and provenance; contains private conversation text.
- [PDF verification](pdf-verification.json) and [artifact checksums](SHA256SUMS.json). PDF verification uses native PDFKit text/page inspection only; no visual rendering or screen capture.
