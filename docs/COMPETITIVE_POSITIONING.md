# Competitive Positioning

Last reviewed: 2026-05-25

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

## Namesake Conflict

As of 2026-05-18, at least three other public projects ship under the
`oh-my-kimi` name and overlap with OMK's category:

| Project | Origin | Overlap |
| --- | --- | --- |
| `oh-my-kimi` (PyPI) | Python package | "Python port of the oh-my-kimi orchestration framework"; uses Kimi native multi-agent (`spawn_agent`) by default. |
| `wang-h/oh-my-kimi` | GitHub | "Kimi-first multi-agent orchestration layer." |
| `Goblin1024/oh-my-kimi` | GitHub | "Workflow orchestration layer for Kimi Code CLI — inspired by oh-my-codex." |

This collision is materially closer to OMK's positioning than Devin, OpenHands,
or Claude Code, and is unresolved. The repository's wedge against these
namesakes is: Rust runtime, durable goal state, explicit verification gates,
`proof.json` semantics, and end-to-end repo delivery with worktrees and
integrator PRs. A naming/priority decision (defend the name vs. rebrand vs.
suffix) is required before the next public release.

## Competitor Map

| Product | Relationship to OMK | Main strength | OMK wedge |
| --- | --- | --- | --- |
| Bernstein | Direct competitor (most similar wedge) | Python orchestrator that drives 40+ CLI coding agents in parallel git worktrees with deterministic scheduling, quality gates, and HMAC-chained audit log. | Rust runtime, durable goal state across crashes, oracle-aware planning, slice-execution with integrator PRs, and proof/failure artifacts as first-class terminal status. |
| CLI Agent Orchestrator (CAO, awslabs) | Direct multi-CLI competitor | Supervisor–worker orchestration over MCP across Claude Code, Kimi CLI, Codex, Gemini CLI, Copilot CLI, OpenCode, Q Developer; isolated tmux sessions; Web UI dashboard. | Goal controller with verification wall and proof semantics rather than a generic supervisor pattern; OMK is opinionated about *what done means*, not just *who runs what*. |
| Kimi Agent SDK (MoonshotAI) | Upstream programmatic competitor | Official SDK for interacting with Kimi CLI programmatically. | Repo-native orchestration layer above the SDK: task graphs, gates, worktrees, PRs, and durable state that the raw SDK does not provide. |
| Devin | Direct hosted competitor | Hosted AI software engineer for scoped coding tasks, PRs, and team workflows. | Local proof-first controller with durable state, explicit gates, and GitHub as output rather than control plane. |
| OpenHands | Direct open-source competitor | AI-driven development platform with CLI, GUI, cloud, and SDK surfaces; ~65k GitHub stars. | Smaller Rust CLI wedge: repo-native task graph, Wire execution, verification wall, and proof bundle. |
| Claude Code | Direct platform competitor | Strong agentic coding surface across terminal, IDE, web, MCP, hooks, skills, and background work. | Engine-adaptable orchestrator with durable goal state and proof semantics that can outlive one assistant session. |
| Cursor background agents | Direct hosted-agent competitor | Async background coding agents inside an IDE-native distribution. | Terminal-native, local-first, engine-adaptable; not tied to a proprietary editor. |
| Aider | Adjacent/direct CLI competitor | Excellent terminal pair-programming, repo map, git-aware edits, and test/lint loop. | Long-running multi-agent goal controller rather than interactive pair-programming loop. |
| Cline | Adjacent local-agent competitor | Local terminal/IDE agent with human-in-the-loop approvals. | Autonomous goal runtime with explicit gates, not a per-step approval loop. |
| SWE-Agent | Adjacent research competitor | Princeton agent framework; standard SWE-bench baseline. | Engineering runtime with delivery semantics, not a benchmark harness. |
| ROMA (Recursive Open Meta-Agent) | Adjacent task-graph competitor | Recursive subtask trees for long-horizon parallel work. | Durable, replayable task graph with gates, worktrees, and PR delivery — not just decomposition. |
| Agyn | Adjacent multi-agent SE competitor | Role-based multi-agent system (coord/research/impl/review). | Equivalent role packs plus durable state, gates, and proof artifacts. |
| GNAP (Git-Native Agent Protocol) | Adjacent protocol competitor | Coordinates AI agents through a handful of JSON files in a git repo, no server. | Same git-native instinct, but OMK ships a full runtime, not only a wire format; should monitor whether to interop. |
| Agenttrace | Adjacent observability competitor | Local-first observability for agent sessions: tokens, cost, latency, tool failures, CI health gates. | Observability is a side-effect of proof artifacts in OMK; should evaluate whether to interop or absorb. |
| Dify | Adjacent workflow competitor | Mature LLM app/workflow builder with agents, RAG, observability, and deployment surfaces. | Software engineering runtime for changing and proving a repository, not building LLM apps. |
| Cody | Adjacent code-context competitor | Enterprise codebase context, search, IDE chat, and code assistant UX. | Execution and verification runtime, not only context and editing help. |
| Codex CLI (OpenAI) | Direct CLI competitor | Official OpenAI terminal coding agent; 85.5k stars, Rust, 100+ commits/mo. | OMK is engine-adaptable and proof-first; Codex is single-agent ChatGPT-integrated chat loop. |
| goose (AAIF) | Direct general-agent competitor | Linux Foundation general-purpose AI agent; desktop + CLI + API; 45.8k stars, Rust, 100+ commits/mo; 15+ providers, 70+ MCP extensions. | OMK is proof-driven engineering runtime; goose is general-purpose agent platform. goose has distribution and governance moat via Linux Foundation. |
| crewAI | Adjacent framework competitor | Enterprise multi-agent automation framework; 52.1k stars, Python, 100+ commits/mo; event-driven flows, cloud control plane. | OMK is local-first CLI runtime; crewAI is cloud-native Python framework. Different surfaces, but competing for multi-agent mindshare. |
| MetaGPT | Adjacent research competitor | Multi-agent software company simulation; 68.3k stars, Python; ICLR 2025 oral; commercial MGX product. | OMK delivers code via worktrees and proof; MetaGPT simulates full SDLC. Different wedge, but high research credibility. |
| Claude Squad | Direct terminal competitor | Go-based TUI for managing multiple agents in tmux workspaces; 7.6k stars, isolated git workspaces, profiles. | OMK has proof semantics and goal decomposition; Claude Squad is agent workspace manager without proof or planning. |
| GitHub Copilot CLI | Direct platform competitor | Official GitHub terminal agent; 10.6k stars; Copilot subscription required; MCP extensibility; autopilot mode. | OMK is free, local, engine-adaptable, and proof-first; Copilot CLI has enormous distribution via GitHub subscriptions. |
| VoltAgent | Adjacent platform competitor | TypeScript AI Agent Engineering Platform; 9.1k stars; framework + cloud console; supervisor/sub-agent, guardrails, evals. | OMK is Rust CLI focused on repo delivery; VoltAgent is general-purpose TS platform with SaaS console. |
| Gemini CLI | Direct CLI competitor | Google's official terminal agent; 104.6k stars, TS, 100+ commits/mo; free tier, 1M context, multimodal, MCP. | OMK is proof-driven multi-agent runtime; Gemini CLI is single-agent Google-integrated chat loop. |
| Qwen Code | Direct CLI competitor | Alibaba's terminal agent; 24.7k stars, TS, 100+ commits/mo; multi-provider, subagents, skills, IDE integration. | OMK has worktree isolation and proof artifacts; Qwen Code is single-agent with subagent skills. |
| Aider | Direct CLI competitor | AI pair programming; 45.3k stars, Python, 6.8M installs; repo map, auto lint/test/fix, git-native. | OMK is goal-first multi-agent controller; Aider is interactive pair-programming loop. Aider sets the bar for terminal UX. |
| AutoGen | Adjacent framework competitor | Microsoft's multi-agent framework; 58.3k stars, Python; maintenance mode, succeeded by MAF. | Legacy threat; monitor Microsoft Agent Framework for enterprise competition. |
| Plandex | Direct planning competitor | Terminal agent for large tasks; 15.4k stars, Go; plan-and-execute, diff sandbox, tree-sitter maps. | OMK has multi-agent orchestration; Plandex is single-agent planning. Diff sandbox and plan VC are market-informed. |
| hcom | Direct local-first competitor | Rust CLI for inter-agent messaging, observation, spawn/fork/resume/kill across terminals; SQLite persistence; 299 stars, very active. | OMK's proof-first goal controller vs. hcom's message-bus coordination; OMK has durable task graph and verification wall, hcom has real-time inter-agent comms. |
| ORCH | Direct multi-agent competitor | TypeScript CLI/TUI for parallel agent teams in isolated git worktrees; mandatory review gate; pre-built team templates; 67 stars, active. | OMK is engine-adaptable with proof semantics; ORCH is Claude Code-centric state machine. |
| Ralph | Adjacent loop-pattern competitor | Bash-based PRD-driven autonomous loop (19.5k stars); fresh instance per iteration; append-only progress log. | OMK has structured runtime, worktrees, and proof artifacts; Ralph is a shell script pattern, not a production runtime. |
| Asynkor | Adjacent coordination competitor | Go-based MCP server for file leasing and cross-machine snapshot sync; prevents edit conflicts at edit time. | OMK uses worktree isolation; Asynkor uses Redis leases. Complementary rather than direct; evaluate interop. |
| ARC Protocol | Adjacent discipline competitor | Python workflow with CONTRACTS.md enforcement and CODEBASE_MAP.md generation; slash-command driven. | OMK has AGENTS.md rules; ARC formalizes contracts and cartography. Market-informed by its discipline-first approach. |
| Claudiomiro | Adjacent pipeline competitor | JavaScript CLI for full pipeline automation (decompose→code→review→test→commit); multi-repo and legacy support. | OMK focuses on proof-driven goal completion; Claudiomiro focuses on parallel full-pipeline execution. |

## Threat Model

| Threat | Level | What it means for OMK |
| --- | --- | --- |
| Namesake `oh-my-kimi` projects already exist on PyPI and GitHub. | High | Brand collision blocks adoption; resolve naming/priority before next release. |
| Bernstein ships parallel-worktree orchestration with gates and audit log. | High | Most overlapping wedge. OMK must differentiate on durable goal state, slice-execution with integrator PRs, and proof terminal semantics — not just "many CLIs in worktrees." |
| CAO (awslabs) becomes the default multi-CLI supervisor. | High | OMK must own *trustable completion semantics*, not just process orchestration. AWS-backed distribution is a real moat risk. |
| Kimi Agent SDK gives upstream programmatic access. | High | The "Kimi-wrapper" layer is now first-party. OMK has to justify itself above the SDK with task graph, gates, and delivery. |
| Devin normalizes "AI software engineer" expectations. | High | OMK must not overpromise; it must answer with proof or precise blockers. |
| Claude Code owns developer distribution. | High | OMK must become a controller layer, not a weaker assistant UI. |
| OpenHands captures open-source agentic development mindshare. | High | OMK must be simpler to run locally and more explicit about proof. |
| Cursor background agents normalize async hosted coding. | Medium | OMK competes by being local-first and editor-agnostic. |
| GNAP becomes a de facto git-native agent protocol. | Medium | Track and decide whether to interop; OMK's git-native instinct aligns. |
| Agenttrace becomes the default agent observability layer. | Medium | Evaluate interop vs. absorption; do not reinvent observability from scratch. |
| Aider sets the bar for terminal editing ergonomics. | Medium | OMK should learn from its fast feedback loop and git-native UX. |
| Dify owns visual workflow/app-builder language. | Medium | OMK should avoid the visual app-builder category. |
| Cody owns enterprise context/search expectations. | Medium | OMK needs strong repo navigation, indexing, and context discipline. |
| hcom becomes the default local agent coordination bus. | High | Direct local-first Rust competitor with messaging, lifecycle, and terminal integration. OMK must differentiate on goal controller semantics, not just agent plumbing. |
| ORCH ships stable multi-agent state machine with mandatory review. | High | Very similar wedge (worktrees, roles, review gate). OMK must own proof artifacts and engine adaptability. |
| Ralph pattern normalizes PRD-driven bash loops. | Medium | High mindshare (19.5k stars). OMK must show that structured runtime beats shell scripts for production work. |
| Asynkor normalizes file leasing for agent teams. | Medium | If file-level coordination becomes expected, OMK may need leases inside worktrees or interop. |
| ARC Protocol normalizes contract-enforced commits. | Medium | OMK's AGENTS.md could evolve into formal contracts; monitor whether this becomes table stakes. |
| Codex CLI becomes the default terminal coding agent. | Critical | 85.5k stars, first-party OpenAI, ChatGPT integration. OMK must differentiate on proof artifacts, multi-agent orchestration, and engine adaptability. |
| goose (AAIF) becomes the default open-source AI agent. | Critical | 45.8k stars, Linux Foundation, multi-provider, MCP-native. OMK must own proof-driven engineering semantics, not general-agent features. |
| GitHub Copilot CLI captures terminal users via subscription. | Critical | 10.6k stars, first-party GitHub, Copilot bundling. OMK competes on being free, local, and proof-first. |
| crewAI captures enterprise multi-agent mindshare. | High | 52.1k stars, enterprise flows, cloud control plane. OMK must own local-first repo-native delivery, not cloud automation. |
| Claude Squad normalizes TUI workspace management. | High | 7.6k stars, Go, tmux-based. OMK should evaluate TUI for multi-agent goal visibility. |
| VoltAgent captures TypeScript agent platform market. | Medium | 9.1k stars, framework + console, supervisor pattern. Adjacent rather than direct; monitor for feature creep into repo delivery. |
| MetaGPT's MGX product competes in natural-language programming. | Medium | Commercial product from 68k-star research project. Monitor for repo-delivery features. |
| Gemini CLI captures terminal users via Google's distribution. | Critical | 104.6k stars, free tier, multimodal, 100+ commits/mo. Largest terminal agent. OMK must differentiate on proof and multi-agent. |
| Qwen Code captures Chinese/international terminal market. | High | 24.7k stars, Alibaba-backed, multi-provider, rapid iteration. Strong international competitor. |
| Aider sets the bar for terminal pair-programming UX. | High | 45.3k stars, 6.8M installs, 15B tokens/week. OMK must match or exceed repo map, auto-lint/test, and git ergonomics. |
| Plandex's plan-and-execute model gains traction. | Medium | 15.4k stars, diff sandbox, tree-sitter maps. Monitor if revived; plan version control is a strong differentiator. |
| AutoGPT normalizes continuous agent platforms. | High | 184.5k stars, marketplace, workflow blocks. OMK must differentiate on bounded goal semantics vs. continuous automation. |
| Dify captures LLM app development mindshare. | High | 142.6k stars, Linux Foundation, visual workflow builder. Adjacent surface but competing for workflow/orchestration mindshare. |
| ChatDev's zero-code platform attracts non-technical users. | Medium | 33.2k stars, NeurIPS 2025, RL orchestrator. Different surface (web GUI), but strong research credibility. |
| PR Agent sets bar for automated code review. | Medium | 11.3k stars, community-owned, multi-platform. OMK proof review must match PR compression and customization. |

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

- create durable goal state under `<omk-state-dir>/goals/<goal-id>/` (XDG: `~/.local/state/omk/goals/`, legacy: `~/.omk/state/goals/`);
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

- Namesake `oh-my-kimi` on PyPI: https://pypi.org/project/oh-my-kimi/
- Namesake `wang-h/oh-my-kimi`: https://github.com/wang-h/oh-my-kimi
- Namesake `Goblin1024/oh-my-kimi`: https://github.com/Goblin1024/oh-my-kimi
- Kimi CLI (upstream): https://github.com/MoonshotAI/kimi-cli
- Kimi Agent SDK (upstream): https://github.com/MoonshotAI/kimi-agent-sdk
- CLI Agent Orchestrator (awslabs): https://github.com/awslabs/cli-agent-orchestrator
- Bernstein (parallel-worktree CLI orchestrator): track via "Bernstein Python orchestrator git worktrees quality gates"
- GNAP (Git-Native Agent Protocol): track via "GNAP git-native agent protocol"
- Agenttrace (local-first agent observability): track via "Agenttrace local agent observability"
- hcom (inter-agent message bus and lifecycle): https://github.com/aannoo/hcom
- ORCH (multi-agent orchestration with worktrees): https://github.com/oxgeneral/ORCH
- Ralph (PRD-driven autonomous loop): https://github.com/snarktank/ralph
- Asynkor (file leasing and cross-machine sync): https://github.com/asynkor/asynkor
- ARC Protocol (contract-enforced workflow): https://github.com/AshishOP/arc-protocol
- Claudiomiro (full pipeline automation): https://github.com/samuelfaj/claudiomiro
- orchestr8 (JIT progressive loading via MCP): https://github.com/seth-schultz/orchestr8
- Codex CLI (OpenAI terminal agent): https://github.com/openai/codex
- goose (AAIF general-purpose agent): https://github.com/aaif-goose/goose
- crewAI (multi-agent framework): https://github.com/crewAIInc/crewAI
- MetaGPT (multi-agent software company): https://github.com/FoundationAgents/MetaGPT
- Claude Squad (multi-agent TUI): https://github.com/smtg-ai/claude-squad
- GitHub Copilot CLI (terminal agent): https://github.com/github/copilot-cli
- VoltAgent (AI Agent Engineering Platform): https://github.com/VoltAgent/voltagent
- Gemini CLI (Google terminal agent): https://github.com/google-gemini/gemini-cli
- Qwen Code (Alibaba terminal agent): https://github.com/QwenLM/qwen-code
- Aider (AI pair programming): https://github.com/Aider-AI/aider
- AutoGen (Microsoft multi-agent framework): https://github.com/microsoft/autogen
- Plandex (terminal agent for large tasks): https://github.com/plandex-ai/plandex
- Shortest (AI E2E testing): https://github.com/antiwork/shortest
- AutoGPT (continuous AI agent platform): https://github.com/Significant-Gravitas/AutoGPT
- Dify (LLM app development platform): https://github.com/langgenius/dify
- ChatDev (zero-code multi-agent platform): https://github.com/OpenBMB/ChatDev
- PR Agent (AI PR review): https://github.com/The-PR-Agent/pr-agent
- Qodo Cover (AI test generation): https://github.com/qodo-ai/qodo-cover
- Bolt.new (in-browser full-stack dev): https://github.com/stackblitz/bolt.new
- Devin docs: https://docs.devin.ai/
- OpenHands: https://github.com/OpenHands/OpenHands
- Claude Code docs: https://code.claude.com/docs/en/overview
- Cursor background agents: https://cursor.com/
- Aider: https://github.com/Aider-AI/aider
- Cline: https://github.com/cline/cline
- SWE-Agent: https://github.com/SWE-agent/SWE-agent
- Dify: https://github.com/langgenius/dify
- Cody docs: https://sourcegraph.com/docs/cody

May 14, 2026 review note: Devin, OpenHands, Claude Code, and Aider still
reinforce the same boundary. OMK should not chase broad hosted-agent or
assistant-surface parity; the defensible MVP remains local durable state,
task-scoped branches/worktrees, explicit verification/review/integration gates,
and proof-backed terminal statuses.

May 25, 2026 review note: competitive intelligence scan (batch 4) added six new tracked competitors. hcom (299★, Rust, 58 commits/mo) is the most credible direct local-first threat; ORCH (67★, active) occupies a nearly identical wedge; Ralph (19.5k★) owns the PRD-loop mindshare; Asynkor (49★, production) defines file-leasing coordination; ARC Protocol (66★) formalizes contract discipline; Claudiomiro (412★) demonstrates legacy-aware multi-repo pipelines. OMK's differentiation remains: durable goal state, proof-first verification wall, engine-adaptable runtime, and slice-execution with integrator PRs. The namesake collision and Bernstein/CAO threats from May 18 remain unresolved.

May 18, 2026 review note: a broader scan surfaced four new high-threat items
not previously tracked. (1) Three public `oh-my-kimi` namesakes — name collision
must be resolved before next release. (2) Bernstein occupies almost exactly the
same wedge (parallel worktrees + quality gates + audit log); OMK's
differentiation is durable goal state, oracle-aware planning, slice-execution
with integrator PRs, and proof terminal status, not feature breadth. (3) AWS's
CAO is the most credible cross-CLI supervisor; OMK competes on completion
semantics, not on running more agents. (4) Moonshot's Kimi Agent SDK collapses
the "thin Kimi wrapper" category; OMK justifies itself above the SDK with task
graph, gates, worktrees, and delivery — not by re-exposing the SDK surface.
GNAP and Agenttrace are medium-threat protocol/observability vectors worth
interop evaluation rather than reimplementation.
