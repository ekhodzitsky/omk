# Module Contract Pattern

Код пишут люди и агенты для будущих людей и агентов. Поэтому модуль должен быть
не просто папкой с файлами, а автономным контекстом разработки: его можно понять,
изменить, проверить и передать дальше без чтения всей системы.

Этот паттерн описывает подход к разработке. Он не зависит от языка,
фреймворка, структуры репозитория или конкретного инструмента тестирования.
Project-specific правила, команды и слои добавляются как adapter поверх этого
ядра.

## Цель

Module Contract Pattern решает четыре проблемы:
- Новый участник не знает, где граница ответственности модуля.
- Изменения ломают скрытые consumers и invariants.
- Документация устаревает, потому что она не связана с verification.
- Агенты создают лишние abstractions, потому что не видят настоящие контракты.

Паттерн требует, чтобы каждый значимый модуль имел короткую machine-readable
карту: что он обещает, от чего зависит, кто его использует, какие инварианты
нельзя ломать, и чем это доказано.

## Core Principle

Модуль является границей владения.

Хороший модуль отвечает на вопросы:
- What does this module own?
- What surface does it expose?
- Who consumes it?
- What dependencies does it rely on?
- What invariants must remain true?
- What proof protects those invariants?
- What verification commands prove the module is still healthy?

Если на эти вопросы нельзя ответить локально, модуль пока не является
автономным контекстом.

## Module Map Format

Рекомендуемый формат: YAML frontmatter + Markdown body.

Frontmatter хранит проверяемый контракт. Markdown body хранит объяснения,
диаграммы, tradeoff notes и примеры. Если проект не использует Markdown, тот же
контракт можно хранить в YAML, JSON, TOML или другом structured format.

````markdown
---
schema_version: 1
module: billing
level: root
layer: domain
purpose: Calculate invoice totals and expose billing decisions to callers.
status: pilot
owners:
  - team: platform
surface:
  - name: InvoiceCalculator
    signature: "InvoiceCalculator<RateProvider>"
    kind: service
    visibility: internal
    contract: Calculates deterministic invoice totals from line items and rates.
    proof:
      kind: unit-test
      target: billing.invoice_calculator.tests.deterministic_totals
      command: test billing.invoice_calculator.tests.deterministic_totals
  - name: InvoiceSummary
    kind: data
    visibility: public
    contract: Stable read model returned to API callers.
    proof:
      kind: contract-test
      target: contracts/billing/invoice_summary.schema
      command: test contracts/billing/invoice_summary.schema
dependencies:
  internal:
    - module: pricing
      scope: rate lookup only
      reason: Billing must use canonical customer rates.
  external:
    - name: decimal arithmetic library
      scope: money calculations
      reason: Floating point is not acceptable for currency.
consumers:
  - path: checkout
    uses: ["InvoiceCalculator.calculate", "InvoiceSummary"]
  - path: reporting
    uses: ["InvoiceSummary"]
invariants:
  - id: deterministic-total
    rule: Same input always produces the same total.
    proof:
      kind: unit-test
      target: billing.invoice_calculator.tests.deterministic_totals
      command: test billing.invoice_calculator.tests.deterministic_totals
  - id: no-floating-point-money
    rule: Money calculations do not use floating point arithmetic.
    proof:
      kind: static-check
      target: billing money implementation
      command: lint no-floating-point-money
verification:
  pre_change:
    - test billing
  full:
    - test all
    - lint all
---

# billing

## Architecture
Explain the design in human terms here. Diagrams are welcome when they clarify
ownership, data flow, or lifecycle.
````

## Schema Fields

| Field | Required | Meaning |
|-------|----------|---------|
| `schema_version` | yes | Version of this contract schema. Start with `1`. |
| `module` | yes | Stable module identifier. Usually folder/package name. |
| `level` | yes | `root` / `subsystem`. Components are described by a parent module. |
| `layer` | recommended | Architecture layer used by import/dependency policy. |
| `purpose` | yes | One sentence describing what this module owns. |
| `status` | yes | `experimental` / `pilot` / `stable` / `deprecated`. |
| `owners` | optional | Humans, teams, or agents responsible for this module. |
| `surface` | yes | Exported or relied-upon surface: name, kind, visibility, contract, proof. |
| `dependencies` | yes | Internal and external dependencies with scope and reason. |
| `consumers` | yes | Known callers, workflows, modules, jobs, services, or users. |
| `invariants` | yes | Rules that must remain true, each linked to proof. |
| `verification` | yes | Fast pre-change checks and full confidence checks. |

`surface` does not mean "make this globally public." It means "this is part of
the module contract." Visibility values are project-defined, for example
`private`, `internal`, `package`, `public`, or `external`.

`signature` is optional. Use it when the bare name loses important information,
such as generics, parameters, protocol version, route shape, or message schema.

`proof` is structured, not prose. It points to the artifact that protects the
contract: test, schema, golden file, fixture, smoke check, static analysis, or
manual verification. If proof does not exist yet, use `kind: missing` so the debt
is visible.

## Levels

Not every folder or package deserves its own module contract.

| Level | Meaning | Contract file? |
|-------|---------|----------------|
| **Root** | Top-level ownership boundary. Other parts of the system know it exists. | Yes |
| **Subsystem** | Meaningful internal boundary inside a root module. Owns a contract worth preserving. | Yes |
| **Component** | Implementation detail below a root/subsystem. | No, unless it becomes an ownership boundary |

Rule: write contracts for ownership boundaries, not for every directory. Too
many tiny contracts create ceremony instead of clarity.

## TODO Files

A module TODO is not a wishlist. It is a short queue of contract work and known
gaps.

```markdown
# <module> TODO

## Current
- [ ] Document module contract.
- [ ] Add proof for exported behavioral contracts.
- [ ] Move side effects behind explicit boundaries where this module owns them.

## Next
- [ ] <concrete task with observable completion>
```

Remove `Current` once the module is compliant. Every TODO item should be
checkable by a reviewer or agent.

## Boundaries And Interfaces

Introduce an interface, trait, protocol, port, adapter, or service boundary when
it solves a concrete problem:
- The module performs I/O, network calls, storage, process execution, or other
  side effects.
- There are multiple real strategies or implementations.
- A lower layer must not depend directly on a higher layer.
- Tests need to exercise logic without using slow or unsafe infrastructure.
- The dependency is volatile, external, expensive, or hard to reproduce.

Do not introduce a boundary only because "interfaces are clean." A plain data
shape, pure function, or internal helper often needs no extra abstraction.

Mocks are evidence that a boundary is useful; they are not, by themselves, the
reason to create one.

## Testing And Proof

Test behavior, not ceremony.

Protect:
- Each exported behavioral contract in `surface`.
- Each invariant in `invariants`.
- Error paths that are part of the contract.
- Compatibility promises such as schemas, wire formats, migrations, or public
  APIs.
- Security, privacy, redaction, idempotency, ordering, or consistency rules when
  they are part of the module's promise.

Usually skip:
- Plain data shapes with no behavior, unless their serialized form is a public
  contract.
- Private helpers that are already covered through the public/local behavior.
- Mechanical re-exports or wrappers with no independent behavior.

Good proof can be a unit test, integration test, contract test, golden file,
schema validation, static check, smoke test, benchmark, or manual runbook. The
important part is that the proof is named and repeatable enough for the project.

## Dependency Policy

Dependencies are part of the contract.

Every dependency should state:
- what is depended on;
- where it is used;
- why it is needed;
- whether it is internal or external;
- what constraint or risk it introduces.

Architecture layers are project-specific, but the rule is universal: dependencies
should point in the intended direction, and exceptions should be explicit,
temporary, and explained.

Example import policy:

| Layer | May depend on | Must not depend on |
|-------|---------------|--------------------|
| `interface` | `application`, `domain` | infrastructure internals |
| `application` | `domain`, declared ports | UI/framework details |
| `domain` | pure shared types | storage, network, UI, framework runtime |
| `infrastructure` | declared ports, external SDKs | UI workflows |

If a project has a validator, `layer` should link the module contract to that
validator's import policy.

## Module Entry File

Many ecosystems have a module entry file: `index.ts`, `mod.rs`, `__init__.py`,
`main.go`, package exports, barrel files, route registries, or manifest exports.

Use the entry file as a showcase, not a storage room:
- declare internal files/submodules;
- re-export intentional surface;
- avoid business logic when the ecosystem allows it;
- keep large behavior in named implementation files.

If the entry file keeps growing, treat it as a design smell. The exact line
threshold is project-specific; use it as a soft signal, not an automatic failure.

## Workflow For A Contributor Or Agent

1. Open the module contract.
2. Read frontmatter first: purpose, surface, dependencies, consumers,
   invariants, verification.
3. Read the Markdown body only for design context needed by the task.
4. Run all commands in `verification.pre_change` when practical.
5. Make the smallest change that satisfies the task and preserves the contract.
6. Add or update proof for any changed contract or invariant.
7. Run all commands in `verification.full`, or record why full verification was
   not possible.
8. Update the contract if surface, dependencies, consumers, invariants, or
   verification changed.
9. Update TODO only with concrete remaining work.

## Minimal Validator

A validator should check:
- `schema_version` is known.
- `module`, `purpose`, `status`, `surface`, `dependencies`, `consumers`,
  `invariants`, and `verification` exist.
- `level` is `root` or `subsystem`.
- `status` is `experimental`, `pilot`, `stable`, or `deprecated`.
- `surface[].name` is stable and machine-readable.
- `surface[].kind`, `visibility`, `contract`, and `proof` exist.
- `proof.kind`, `proof.target`, and `proof.command` exist.
- `proof.kind` belongs to the project's known proof kinds, for example
  `unit-test`, `integration-test`, `contract-test`, `golden`, `schema`,
  `smoke`, `static-check`, `benchmark`, `manual`, or `missing`.
- `proof.command` is non-empty unless `kind` is `manual` or `missing`.
- Internal dependencies include `module`, `scope`, and `reason`.
- External dependencies include `name`, `scope`, and `reason`.
- Each invariant has a stable `id`, a human-readable `rule`, and proof.
- `verification.pre_change` and `verification.full` are non-empty lists.

The validator should permit `kind: missing` during migration, but should report
it as visible debt.

## Adapters

The core pattern is language-neutral. Adapters translate it into project
conventions.

Rust adapter examples:
- `kind`: `trait`, `struct`, `enum`, `fn`, `module`.
- `visibility`: `private`, `pub(crate)`, `pub`.
- `verification`: `cargo test`, `cargo clippy`, `cargo doc`.
- Entry file: `mod.rs` or `lib.rs`.

TypeScript adapter examples:
- `kind`: `type`, `interface`, `class`, `function`, `module`, `route`.
- `visibility`: `private`, `package`, `exported`, `public-api`.
- `verification`: `npm test`, `npm run typecheck`, `npm run lint`.
- Entry file: `index.ts` or package export map.

Python adapter examples:
- `kind`: `class`, `function`, `protocol`, `module`, `schema`.
- `visibility`: `private`, `package`, `public`.
- `verification`: `pytest`, `ruff`, `pyright`, `mypy`.
- Entry file: `__init__.py` or package module.

Service/API adapter examples:
- `kind`: `endpoint`, `event`, `job`, `message`, `schema`, `workflow`.
- `visibility`: `internal`, `partner`, `public`, `deprecated`.
- `proof`: contract test, OpenAPI schema check, protobuf/golden compatibility,
  smoke test, replay test.
- Verification: service tests, migration checks, canary checks, schema registry
  compatibility.

## Anti-Patterns

Avoid:
- Contract files for every tiny implementation directory.
- Interfaces created only to look architectural.
- Proof described in prose but not tied to an artifact.
- Verification commands that nobody can run.
- Consumers listed as "various" or "unknown" when they can be discovered.
- Dependencies listed without a reason.
- Architecture diagrams treated as proof.
- TODO files that accumulate vague future ideas.

The pattern is successful when it makes a module easier to change safely, not
when it produces more documentation.
