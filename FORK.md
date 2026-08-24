# AsheTheWings Codex fork

This repository is a release-based fork of
[OpenAI Codex](https://github.com/openai/codex). It preserves a small set of
intentional product differences on top of reviewed official releases. It does
not track the moving upstream development branch. `FORK.yaml` is the
machine-readable provenance record; this file is the curated divergence ledger
and maintenance guide.

The inherited README and `CHANGELOG.md` describe upstream OpenAI distribution
channels. They are not a record of this fork's releases or differences.

## Provenance and compatibility

| Field | Value |
| --- | --- |
| Upstream | [`openai/codex`](https://github.com/openai/codex) |
| Official release | [`rust-v0.149.1`](https://github.com/openai/codex/releases/tag/rust-v0.149.1) |
| Baseline commit | [`ff29a44391deccde0aba0f8390337d7f3c319ea4`](https://github.com/openai/codex/commit/ff29a44391deccde0aba0f8390337d7f3c319ea4) |
| Fork integration branch | `main` |
| Compatibility boundary | Reviewed official Codex release tags |
| License | [Apache-2.0](LICENSE) |

Official release tags are this fork's compatibility boundary. Fork changes are
replayed onto a reviewed official release, verified together, and recorded here
before that release becomes the new baseline. The fork does not merge or claim
compatibility with the moving upstream `main` branch.

To inspect the complete repository delta rather than only the curated product
differences below:

```shell
baseline=ff29a44391deccde0aba0f8390337d7f3c319ea4
git log --oneline "$baseline"..main
git diff --stat "$baseline"..main
```

Git history is the implementation record. This ledger intentionally excludes
ordinary delivery metadata, tests that only support another change, and
documentation-only commits.

## Using this fork

The fork does not currently publish its own binary releases. The installer,
npm, Homebrew, and release links in the inherited README install official
OpenAI builds and therefore do not contain the differences listed here.

Build and run the fork from source instead:

```shell
git clone https://github.com/AsheTheWings/codex.git
cd codex/codex-rs
cargo run --bin codex
```

For prerequisites, optimized builds, and project validation commands, see
[`docs/install.md`](docs/install.md). Use this fork's clone URL in place of the
upstream URL shown there.

## Divergence status model

- **proposed**: implemented on an open pull request but not present on `main`;
- **active**: intentionally maintained on `main`;
- **upstreamed**: equivalent behavior has landed in an official release and is
  awaiting or has completed removal from the fork;
- **superseded**: replaced by another fork behavior; and
- **retired**: deliberately removed without an equivalent replacement.

Only `active` entries describe behavior available from the fork's `main`
branch.

## Active product differences

### F001 — Render terminal commands individually

- **Status:** active
- **Behavior:** Successful terminal commands and their output remain visible as
  individual transcript cells instead of being collapsed into a single
  `Ran command` summary. Exploration grouping, replay, and event ordering are
  preserved.
- **Rationale:** Command-level visibility makes the agent's actions and output
  auditable from the TUI transcript.
- **Release replay:** commit
  [`634ee0a04`](https://github.com/AsheTheWings/codex/commit/634ee0a047be21d3a2989cdfcbf6ed858aabcec8)
- **Original fork source:** commit
  [`bf4da507b`](https://github.com/AsheTheWings/codex/commit/bf4da507b048445a5864c551c7ab7df463c80121),
  [PR #1](https://github.com/AsheTheWings/codex/pull/1)
- **Conflict hotspots:** `codex-rs/tui/src/exec_cell` and
  `codex-rs/tui/src/chatwidget/tests/exec_flow.rs`
- **Verification:** TUI exec-flow tests and rendering snapshots cover command
  completion, replay, grouping, and ordering.
- **Upstream/removal condition:** no upstream proposal is tracked. Retire this
  delta only if an official release provides equivalent command-level
  transcript visibility or the fork deliberately adopts upstream compaction.

### F002 — Configure earlier-prompt editing

- **Status:** active
- **Behavior:** `tui.prompt_edit_mode` selects how editing an earlier prompt
  continues the conversation. It defaults to `overwrite`; users can choose
  `branch` to preserve the original thread and continue in a fork.
- **Rationale:** In-session overwrite preserves the established editing model
  while retaining upstream branching as an explicit option.
- **Release replay:** commit
  [`2ec94e0e0`](https://github.com/AsheTheWings/codex/commit/2ec94e0e045a0b5fa9443e79756bb457132b3d49)
- **Original fork sources:** commits
  [`4e1fd0286`](https://github.com/AsheTheWings/codex/commit/4e1fd0286f3db403db328d28feebc537cc747894)
  and
  [`3a01f1fba`](https://github.com/AsheTheWings/codex/commit/3a01f1fba9f139683cc5cb03b142b7a10326906b),
  [PR #2](https://github.com/AsheTheWings/codex/pull/2) and
  [PR #3](https://github.com/AsheTheWings/codex/pull/3)
- **Conflict hotspots:** the configuration schema, TUI prompt-edit and
  backtrack flows, app-server session integration, core session handlers, and
  thread-store live-writer ownership.
- **Verification:** configuration, TUI, thread-store, and app-server tests cover
  both modes, atomic durable replacement, subsequent appends, paginated
  history, and session-scoped runtime preservation.
- **Upstream/removal condition:** no upstream proposal is tracked. Retire or
  narrow this delta when an official release offers equivalent configuration
  with a compatible default and session lifecycle.

### F003 — Reject incompatible implicit local daemons

- **Status:** active
- **Behavior:** A source-built TUI reuses an implicitly discovered local app
  server only when its version exactly matches the TUI. On version mismatch it
  disconnects and uses the embedded app server. Explicit remote endpoints are
  unaffected.
- **Rationale:** Reusing an installed daemon with an older RPC surface can make
  newer requests such as `thread/revert` fail even though the source-built TUI
  supports them.
- **Release replay:** commit
  [`4041cefab`](https://github.com/AsheTheWings/codex/commit/4041cefabdb31bf09e163b3db76efd1610beee7d)
- **Original fork source:** commit
  [`0d2035ae9`](https://github.com/AsheTheWings/codex/commit/0d2035ae9ed63a5461fc6898f902ace45baaa919),
  [PR #3](https://github.com/AsheTheWings/codex/pull/3)
- **Conflict hotspots:** implicit app-server discovery and connection setup in
  `codex-rs/tui/src/lib.rs`.
- **Verification:** the local-daemon version-match unit test covers matching,
  mismatching, and missing versions.
- **Upstream/removal condition:** no upstream proposal is tracked. Retire this
  delta if an official release makes implicit local-daemon reuse safe across
  RPC versions or provides an equivalent exact-version fallback.

### F004 — Revert paginated threads in place

- **Status:** active
- **Behavior:** In-place edits of paginated threads replace durable rollout
  history without replacing the live session, preserving MCP connections and
  other session-scoped runtimes.
- **Rationale:** Editing conversation history should not restart unrelated
  runtime services.
- **Release replay:** incorporated into commit
  [`2ec94e0e0`](https://github.com/AsheTheWings/codex/commit/2ec94e0e045a0b5fa9443e79756bb457132b3d49)
- **Original fork source:** commit
  [`3a01f1fba`](https://github.com/AsheTheWings/codex/commit/3a01f1fba9f139683cc5cb03b142b7a10326906b),
  [PR #3](https://github.com/AsheTheWings/codex/pull/3)
- **Conflict hotspots:** app-server thread processing, core session handlers,
  protocol thread-revert messages, and thread-store live-writer ownership.
- **Verification:** thread-store revert tests and app-server v2 thread-revert
  and MCP tests cover atomic replacement, subsequent appends, and runtime
  preservation.
- **Upstream/removal condition:** no upstream proposal is tracked. Retire this
  delta if an official release provides an equivalent in-place paginated revert
  that retains session-scoped runtimes.

## Recording a divergence

Add an entry only for an intentional, externally meaningful difference from
the recorded release baseline. Give it a stable `F###` identifier and record:

1. status, intended behavior, and rationale;
2. release-replay and original-source commits;
3. affected subsystem and likely release-upgrade conflict hotspots;
4. the focused verification that protects the behavior;
5. an upstream issue or pull request when one exists; and
6. a concrete upstreaming, replacement, or removal condition.

Update an existing entry's status rather than deleting its history. Keep
implementation chronology in Git and release-to-release changes in fork
release notes if this fork starts publishing versioned binaries.

## Advancing the baseline

Advance the fork only to an official Codex release that has been deliberately
reviewed. For each upgrade:

1. verify the official release tag and record its exact commit;
2. create the candidate history from that tag rather than upstream `main`;
3. replay the project manifest and active fork changes in ledger order;
4. resolve conflicts without silently dropping active behavior;
5. run focused tests for every conflict hotspot and the normal affected-package
   checks, then build the release binary;
6. update `FORK.yaml`, this file, and the README notice to the reviewed tag and
   commit; and
7. submit the complete release replay for review before replacing the fork's
   integration history.

Never merge a moving upstream branch into the fork. A release upgrade is a
reviewed replay onto a new, immutable compatibility boundary.
