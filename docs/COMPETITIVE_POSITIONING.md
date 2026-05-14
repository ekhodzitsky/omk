# Competitive Positioning

Last reviewed: 2026-05-14

This document is the canonical market map for `omk goal`.

OMK should not drift into "another AI chat for code" or "another visual LLM app
builder." The target category is narrower and harder:

> Local, repo-native, proof-driven autonomous software engineering runtime.

## Positioning Contract

`omk goal` must be positioned around six non-negotiable claims:

1. **Local-first.** The repository, git history, state, task graph, artifacts,
   and proof live on the user's machine by default.
2. **Repo-native.** OMK works through branches, worktrees, commits, tests,
   changelogs, release notes, and GitHub output.
3. **Proof-first.** A run is not done because an agent sounds confident. It is
   done only when gates, reviews, and proof artifacts support the claim.
4. **Goal-first.** The main UX is one high-level outcome, not a chat loop or
   hand-built visual workflow.
5. **Long-horizon and resumable.** Goal state, task graph, heartbeats, events,
   budgets, and artifacts must survive process crashes and context loss.
6. **Engine-adaptable.** Kimi/Wire is the current execution engine, but the
   controller should be designed so future engines can be adapters, not rewrites.

Short product copy:

```text
OMK is a local, proof-driven engineering runtime that turns one high-level goal
into planned, agent-executed, verified repository changes.
```

## Inspired By vs Market-Informed

Use these terms carefully.

- **Inspired by:** only use for direct lineage or explicit design borrowing. The
  current public README correctly says OMK is inspired by `oh-my-claudecode`.
- **Market-informed by:** use for competitors and benchmarks that shape our
  positioning but are not design parents.
- **Compared with:** use when explaining why OMK's wedge is different.

Do not say OMK is "inspired by Devin," "inspired by Claude Code," or "inspired
by OpenHands" in public copy. Those tools are competitors and benchmarks. The
safe phrasing is:

```text
OMK is inspired by oh-my-claudecode and market-informed by the broader agentic
coding landscape, including Devin, OpenHands, Claude Code, Aider, Dify, and Cody.
```

No competitor code, prompts, assets, branding, or proprietary workflows should be
copied into OMK.

## Competitor Map

| Product | Relationship to OMK | Main strength | OMK wedge |
| --- | --- | --- | --- |
| Devin | Direct competitor | Hosted AI software engineer for scoped coding tasks, PRs, and team workflows. | Local proof-first controller with durable state, explicit gates, and GitHub as output rather than control plane. |
| OpenHands | Direct open-source competitor | AI-driven development platform with CLI, GUI, cloud, and SDK surfaces. | Smaller Rust CLI wedge: repo-native task graph, Wire execution, verification wall, and proof bundle. |
| Claude Code | Direct platform competitor | Strong agentic coding surface across terminal, IDE, web, MCP, hooks, skills, and background work. | Engine-adaptable orchestrator with durable goal state and proof semantics that can outlive one assistant session. |
| Aider | Adjacent/direct CLI competitor | Excellent terminal pair-programming, repo map, git-aware edits, and test/lint loop. | Long-running multi-agent goal controller rather than interactive pair-programming loop. |
| Dify | Adjacent workflow competitor | Mature LLM app/workflow builder with agents, RAG, observability, and deployment surfaces. | Software engineering runtime for changing and proving a repository, not building LLM apps. |
| Cody | Adjacent code-context competitor | Enterprise codebase context, search, IDE chat, and code assistant UX. | Execution and verification runtime, not only context and editing help. |

## Threat Model

| Threat | Level | What it means for OMK |
| --- | --- | --- |
| Devin normalizes "AI software engineer" expectations. | High | OMK must not overpromise; it must answer with proof or precise blockers. |
| Claude Code owns developer distribution. | High | OMK must become a controller layer, not a weaker assistant UI. |
| OpenHands captures open-source agentic development mindshare. | High | OMK must be simpler to run locally and more explicit about proof. |
| Aider sets the bar for terminal editing ergonomics. | Medium | OMK should learn from its fast feedback loop and git-native UX. |
| Dify owns visual workflow/app-builder language. | Medium | OMK should avoid the visual app-builder category. |
| Cody owns enterprise context/search expectations. | Medium | OMK needs strong repo navigation, indexing, and context discipline. |

## Strategic Boundary

OMK should not compete by becoming:

- a generic hosted coding agent;
- a visual LLM app builder;
- an IDE-only autocomplete/chat product;
- a model-provider marketplace;
- a one-shot code generator;
- an unbounded recursive agent launcher.

OMK should compete by becoming:

- a durable goal controller;
- a local task graph and worker runtime;
- a verification wall around code changes;
- a proof artifact generator;
- a GitHub output producer;
- a policy layer over multiple possible execution engines.

## MVP Competitive Slice

The first `omk goal` MVP should win a narrow use case:

```bash
omk goal run "fix this repository until tests and proof pass" --until-ready
```

Minimum competitive requirements:

- create durable goal state under `.omk/goals/<goal-id>/`;
- write PRD, technical plan, test spec, and task graph;
- execute bounded tasks through current Wire/team primitives;
- run configured gates;
- produce `proof.json` or `failure.json`;
- report one truthful terminal status;
- resume or explain why it cannot resume.

This is intentionally smaller than Devin, Claude Code, or OpenHands. The wedge is
not feature breadth; the wedge is trustable completion semantics.

## Product Language

Preferred:

- "proof-driven autonomous engineering runtime"
- "local goal controller for repository work"
- "one command, durable task graph, verified output"
- "ready, not ready, or blocked with evidence"

Avoid:

- "magic overnight engineer"
- "Devin clone"
- "Claude Code alternative" as the primary identity
- "fully autonomous production-ready software for any idea"
- "unlimited agents"

## Watchlist

Re-check these before major `omk goal` releases:

- Devin docs: https://docs.devin.ai/
- OpenHands: https://github.com/OpenHands/OpenHands
- Claude Code docs: https://code.claude.com/docs/en/overview
- Aider: https://github.com/Aider-AI/aider
- Dify: https://github.com/langgenius/dify
- Cody docs: https://sourcegraph.com/docs/cody

May 14, 2026 review note: Devin, OpenHands, Claude Code, and Aider still
reinforce the same boundary. OMK should not chase broad hosted-agent or
assistant-surface parity; the defensible MVP remains local durable state,
task-scoped branches/worktrees, explicit verification/review/integration gates,
and proof-backed terminal statuses.
