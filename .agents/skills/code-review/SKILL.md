---
name: code-review
description: >-
  Perform an adversarial code review of a J-SYN pull request, branch, or commit
  against AGENTS.md, .github/copilot-instructions.md, and docs/FX_UNIT_SPEC.md,
  and post notable findings to the PR via the gh CLI. Use when the user invokes
  /code-review or asks for an adversarial review of a PR or branch.
---

# Adversarial Code Review Workflow (J-SYN)

## Invocation & Syntax

Supports flexible natural arguments after `/code-review`:

- `/code-review` — auto-detect open PR for the current branch (or diff `main...HEAD` locally).
- `/code-review #39` or `/code-review pr 39` — review PR `#39`.
- `/code-review <target> [focus] [mode]`:
  - **Focus (optional):** `dsp` (RT audio safety, math/clamps, sample-rate invariance), `ui` (Vizia/femtovg, tokens, widget math, FX Unit spec), `presets` (IDs, enum migrations, v1/v2 roundtrips). Defaults to full review.
  - **Mode (optional):** `local` / `--no-post` (chat summary only, do not post to GitHub), `fix` (also propose/apply fixes after review). Defaults to posting a `gh pr comment` when notable findings exist.

---

## Step-by-Step Procedure

1. **Load Project Standards (Always Read First):**
   - Read `AGENTS.md`, `.github/copilot-instructions.md`, and (if any UI/FX files changed) `docs/FX_UNIT_SPEC.md`.
   - Respect §5 ("Known Intentional Idioms") of `.github/copilot-instructions.md` so intentional idioms (`permit_alloc` around `ArcSwap::load()`, `ParamPtr` unsafe blocks with safety comments, documented fast math) are never flagged as false positives.

2. **Fetch Target Diff & Metadata:**
   - For a PR number (`N`), run `gh pr view N --json number,title,body,baseRefName,headRefName,state,url,comments,reviews` and `gh pr diff N` (or `git diff main...HEAD` if already checked out on that branch).
   - For the current branch with no PR number, check `gh pr view --json number,url` to see if an open PR exists; otherwise diff `main...HEAD`.

3. **Adversarial Audit (Verify Every Claim Against Source):**
   Never review from the diff alone — read surrounding call sites, helper definitions, and widget/DSP implementations for every touched symbol. Check:
   - **Real-Time Audio Safety (`j-syn-dsp`):** Zero heap allocs *and* zero drops/deallocs on the audio thread; headless `JSynBgTask` provisioning; denormal/NaN/Inf guards; block-size & sample-rate invariance; no coupling of `fx/` DSP to global synth flags.
   - **Math, Clamping & Boundary Conditions:** Verify UI display formatting matches DSP math 1:1 across the entire parameter range and every mode combination (check min/max endpoints, integer step counts such as `ParamStepPill` `num_steps` = distinct value count, and cross-mode state transitions like `MODSEQ` <-> `LFO` or DAW automation).
   - **Preset & Parameter Stability:** Immutable `#[id = "..."]` strings; enum additions/renames paired with `presets.rs` v1/v2 migration & `enum_name_alias`; `with_value_to_string` paired with `with_string_to_value`; PDC latency reported only in `initialize()`.
   - **Vizia GUI, Theme & FX Unit Hygiene:** Single 60 Hz timer; `ModValChanged` (not `ModUiChanged`) during continuous drags; no unsupported CSS selectors (`:focus-within`), no `.z_index` inside `ScrollView` / `fx_rack/`, no Unicode arrows inside `vizia::Button`, no `.hoverable(false)` wrapping interactive overlays, bound `femtovg` font before `fill_text`, theme-only colors, `tokens::*`, `FX Unit` naming (never `snapin`).
   - **Maintainability & Tests:** Flag duplicated boilerplate, silent fallbacks, `>150` lines added to `>800`-line files (suggesting a concrete extraction), and missing invariant-checking unit tests for new edge cases.

4. **Report & Post Findings:**
   - Classify each verified issue as **Blocking** (with exact `file:line`, concrete failure scenario/math, and minimal fix) or **Maintainability / Non-Blocking**.
   - If reviewing an open PR and notable findings exist (and `local` / `--no-post` was not requested), post a concise markdown comment via `gh pr comment <N> --body ...` and return the comment URL.
   - If no notable issues are found, state what was verified (RT safety, presets, UI/DSP parity, tests) and skip posting noise unless the user asks for an explicit LGTM comment.
