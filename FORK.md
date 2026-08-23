# AsheTheWings Codex fork

This repository is a maintained fork of
[OpenAI Codex](https://github.com/openai/codex). It tracks upstream development
while preserving a small set of intentional product differences. `FORK.yaml`
is the machine-readable provenance record; this file is the curated divergence
ledger and maintenance guide.

The inherited README and `CHANGELOG.md` describe upstream OpenAI distribution
channels. They are not a record of this fork's releases or differences.

## Provenance and synchronization

| Field | Value |
| --- | --- |
| Upstream | [`openai/codex`](https://github.com/openai/codex) |
| Upstream branch | `main` |
| Fork integration branch | `main` |
| Last synchronized upstream commit | [`83d1fe0e67b1323f71febc2925817732b449f1d9`](https://github.com/openai/codex/commit/83d1fe0e67b1323f71febc2925817732b449f1d9) |
| Last synchronization date | 2026-08-23 |
| License | [Apache-2.0](LICENSE) |

The synchronization commit is the last upstream commit in the fork's
integration history, not a claim that the moving upstream branch is currently
at that commit. Every upstream-sync pull request must update `FORK.yaml` and
this table to the exact upstream commit it integrates.

To inspect the complete repository delta rather than only the curated product
differences below:

```shell
baseline=83d1fe0e67b1323f71febc2925817732b449f1d9
git log --oneline "$baseline"..main
git diff --stat "$baseline"..main
```

Git history is the implementation record. This ledger intentionally excludes
ordinary maintenance, delivery metadata, tests that only support another
change, and documentation-only commits.

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
- **upstreamed**: equivalent behavior has landed upstream and is awaiting or
  has completed removal from the fork;
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
- **Introduced by:** commit
  [`bf4da507b`](https://github.com/AsheTheWings/codex/commit/bf4da507b048445a5864c551c7ab7df463c80121),
  [PR #1](https://github.com/AsheTheWings/codex/pull/1)
- **Conflict hotspots:** `codex-rs/tui/src/exec_cell` and
  `codex-rs/tui/src/chatwidget/tests/exec_flow.rs`
- **Verification:** TUI exec-flow tests and rendering snapshots cover command
  completion, replay, grouping, and ordering.
- **Upstream/removal condition:** no upstream proposal is tracked. Retire this
  delta only if upstream provides equivalent command-level transcript
  visibility or the fork deliberately adopts upstream compaction.

### F002 — Configure earlier-prompt editing

- **Status:** active
- **Behavior:** `tui.prompt_edit_mode` selects how editing an earlier prompt
  continues the conversation. It defaults to `overwrite`; users can choose
  `branch` to preserve the original thread and continue in a fork.
- **Rationale:** In-session overwrite preserves the established editing model
  while retaining upstream branching as an explicit option.
- **Introduced by:** commit
  [`4e1fd0286`](https://github.com/AsheTheWings/codex/commit/4e1fd0286f3db403db328d28feebc537cc747894),
  [PR #2](https://github.com/AsheTheWings/codex/pull/2)
- **Conflict hotspots:** the configuration schema, TUI prompt-edit and
  backtrack flows, and app-server session integration.
- **Verification:** configuration parsing/schema tests and TUI prompt-edit,
  backtrack, failure-recovery, and paginated-thread tests cover both modes.
- **Upstream/removal condition:** no upstream proposal is tracked. Retire or
  narrow this delta when upstream offers equivalent configuration with a
  compatible default.

### F003 — Reject incompatible implicit local daemons

- **Status:** active
- **Behavior:** A source-built TUI reuses an implicitly discovered local app
  server only when its version exactly matches the TUI. On version mismatch it
  disconnects and uses the embedded app server. Explicit remote endpoints are
  unaffected.
- **Rationale:** Reusing an installed daemon with an older RPC surface can make
  newer requests such as `thread/revert` fail even though the source-built TUI
  supports them.
- **Introduced by:** commit
  [`0d2035ae9`](https://github.com/AsheTheWings/codex/commit/0d2035ae9ed63a5461fc6898f902ace45baaa919),
  [PR #3](https://github.com/AsheTheWings/codex/pull/3)
- **Conflict hotspots:** implicit app-server discovery and connection setup in
  `codex-rs/tui/src/lib.rs`.
- **Verification:** the local-daemon version-match unit test covers matching,
  mismatching, and missing versions.
- **Upstream/removal condition:** no upstream proposal is tracked. Retire this
  delta if upstream makes implicit local-daemon reuse safe across RPC versions
  or provides an equivalent exact-version fallback.

### F004 — Revert paginated threads in place

- **Status:** active
- **Behavior:** In-place edits of paginated threads replace durable rollout
  history without replacing the live session, preserving MCP connections and
  other session-scoped runtimes.
- **Rationale:** Editing conversation history should not restart unrelated
  runtime services.
- **Introduced by:** commit
  [`3a01f1fba`](https://github.com/AsheTheWings/codex/commit/3a01f1fba9f139683cc5cb03b142b7a10326906b),
  [PR #3](https://github.com/AsheTheWings/codex/pull/3)
- **Conflict hotspots:** app-server thread processing, core session handlers,
  protocol thread-revert messages, and thread-store live-writer ownership.
- **Verification:** thread-store revert tests and app-server v2 thread-revert
  and MCP tests cover atomic replacement, subsequent appends, and runtime
  preservation.
- **Upstream/removal condition:** no upstream proposal is tracked. Retire this
  delta if upstream provides an equivalent in-place paginated revert that
  retains session-scoped runtimes.

## Recording a divergence

Add an entry only for an intentional, externally meaningful difference from
upstream. Give it a stable `F###` identifier and record:

1. status, intended behavior, and rationale;
2. introducing commit and pull request;
3. affected subsystem and likely upstream-sync conflict hotspots;
4. the focused verification that protects the behavior;
5. an upstream issue or pull request when one exists; and
6. a concrete upstreaming, replacement, or removal condition.

Update an existing entry's status rather than deleting its history. Keep
implementation chronology in Git and release-to-release changes in fork release
notes if this fork starts publishing versioned binaries.

## Synchronizing upstream

Upstream changes are integrated through a dedicated branch and pull request;
the customized `main` branch is not a pristine upstream mirror. A typical sync
starts with:

```shell
git remote add upstream https://github.com/openai/codex.git  # first sync only
git fetch origin main
git fetch upstream main
git worktree add -b chore/sync-upstream-YYYYMMDD \
  ../codex-sync-YYYYMMDD origin/main
cd ../codex-sync-YYYYMMDD
git merge --no-ff upstream/main
```

During the sync:

1. resolve conflicts without silently dropping active fork behavior;
2. run focused tests for every conflict hotspot and the normal affected-package
   checks;
3. review each ledger entry against the new upstream behavior;
4. update `FORK.yaml` and this file to the integrated upstream commit and date;
5. commit any conflict resolutions or metadata updates with the sync branch;
   and
6. push the branch and merge it through a reviewed pull request.

After the pull request merges, compare the resulting `main` branch with the
recorded upstream commit and confirm that every remaining product difference is
represented by an `active` ledger entry.
