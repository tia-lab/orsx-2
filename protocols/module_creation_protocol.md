--------------------------------------------------------------------------------
MATHILDE PROPRIETARY AND CONFIDENTIAL
Copyright (c) 2024 MATHILDE. All Rights Reserved.

This document contains trade secrets and confidential information owned
exclusively by MATHILDE, protected under Swiss law (URG, UWG, Art. 162 StGB).

PROHIBITED: Reproduction, copying, distribution, disclosure, or derivative
works without prior written authorization from MATHILDE.

ACCESS REQUIREMENT: Executed NDA with MATHILDE required. Unauthorized access
or possession violates Swiss law. Violations subject to civil remedies,
injunctive relief, damages, and criminal prosecution.

Legal Contact: massimo.nicora@wnlegal.ch
--------------------------------------------------------------------------------

## MANDATORY LEGAL HEADER FOR GENERATED DOCUMENTS

**ENFORCEMENT REQUIREMENT**: Any document generated using this protocol MUST include the following legal header at the very top, copied EXACTLY as shown. NO MODIFICATIONS ALLOWED.

```
--------------------------------------------------------------------------------
MATHILDE PROPRIETARY AND CONFIDENTIAL
Copyright (c) 2024 MATHILDE. All Rights Reserved.

This document contains trade secrets and confidential information owned
exclusively by MATHILDE, protected under Swiss law (URG, UWG, Art. 162 StGB).

PROHIBITED: Reproduction, copying, distribution, disclosure, or derivative
works without prior written authorization from MATHILDE.

ACCESS REQUIREMENT: Executed NDA with MATHILDE required. Unauthorized access
or possession violates Swiss law. Violations subject to civil remedies,
injunctive relief, damages, and criminal prosecution.

Legal Contact: massimo.nicora@wnlegal.ch
--------------------------------------------------------------------------------
```

**This is a mandatory requirement. Documents without this exact header are non-compliant.**

---

# PROTOCOL: Shared Module Creation (RESEARCH -> SPEC -> IMPLEMENT -> TEST -> READY)

Version: 1.0
Type: Engineering + Research Protocol
Location: /v2/crates/math/docs/protocols/module_creation_protocol.md
Created: 2026-01-09
Author: Protocol Draft (Codex) | Requires: CEO Approval

---

## INDEROGABLE RULES

```
1. INTELLECTUAL HONESTY AND MATHEMATICAL RIGOR ARE MANDATORY
2. ALWAYS PROVE THE ASSUMPTION
3. NEVER LIE OR TWEAK RESPONSE
4. NEVER WRITE SOMETHING THAT IS NOT PROVED
5. NEVER USE ICONS OR EMOJIS
6. CLARITY OVER VERBOSITY
7. NEVER USE HYPERBOLIC WORDS OR MARKETING LANGUAGE
8. ALWAYS USE GROUNDED AND HUMBLE TONE
9. MATHILDE MEASURES, NOT PREDICTS
10. DETERMINISM IS MANDATORY (cron + reproducibility)
11. PERFORMANCE IS TIME-BOUNDED (fastest possible without losing correctness)
12. ZERO DUPLICATION INSIDE /v2/crates/math/src/ IS MANDATORY
13. NEVER RELAX TEST TOLERANCES BEFORE PROVING THE ALGORITHM IS CORRECT
14. NO LATEX FORMULA FOR MARKDOWN FILE WE USE MARKDOWN ENCODING
```

**VIOLATION OF ANY RULE = PROTOCOL RESTART**

---

## PURPOSE

Define a single standard process to add or extend a shared module under:

- `/v2/crates/math/src/`

Shared modules are infrastructure. They must be reusable, deterministic, numerically stable, performance-bounded, and tested independently of any single indicator.

This protocol is called BEFORE writing code.

---

## MATHILDE CONTEXT (MANDATORY)

This protocol exists to protect production constraints and financial correctness:

- MATHILDE is a measurement layer, not an opinion or prediction layer.
- Outputs must be computable from information available up to time t (measurement-only).
- The system runs deterministically under time-bounded cron execution; performance budgets are hard requirements.
- Crypto markets require explicit adaptation (24/7, fat tails, wicks/gaps, exchange microstructure effects).
- Financial computation requires explicit assumptions, numerical stability, and explicit failure contracts.

---

## ENFORCEMENT: PROTOCOL INVOCATION (BLOCKING)

This protocol cannot be executed on an undefined module.

Before Phase 0 begins, the requester (CEO/PHD/ARCH) MUST provide the minimum "module intake" inputs:

- Proposed module name and location under `/v2/crates/math/src/` (or "TBD" with a concrete domain folder).
- One-sentence statement: "What does it compute/measure?"
- Why this belongs in `/v2/crates/math/` (2+ planned consumers OR foundational primitive justification).
- Input type and scale: event-based or bar-based, expected maximum sizes (bounds), and the units of time or indexing.

### CHECKPOINT -1: Intake Completeness (STOP if fail)

- [ ] Module intake inputs provided (no assumptions)
- [ ] Measurement framing is explicit (no predictive language)
- [ ] Crypto/financial relevance is explicit (what market object it measures/supports)

---

## SCOPE AND NON-SCOPE

**IN SCOPE**:

- New module folder under `/v2/crates/math/src/`
- Major extension of an existing shared module (new algorithm, new API)
- Shared primitives added to `math/src/*` (and any common math helpers), etc.

**OUT OF SCOPE** (use indicator protocols instead):

- A single indicator implementation (use `math_specs.md` + `testing_protocol.md`)
- Minor refactors inside an existing shared module that do not change behavior (still must keep determinism/stability)

---

## DEFINITIONS (MANDATORY)

- **Module**: reusable algorithmic component intended to be used by 2+ indicators or processors.
- **Deterministic output (epsilon-based)**: the same inputs, parameters, and build produce outputs that are equal within explicit tolerances (no bitwise requirement).
- **Time-bounded**: every exported function has an explicit maximum runtime profile given explicit input size bounds; iterative methods must have max-iteration caps.
- **Failure contract**: a precise specification of invalid inputs, non-convergence, and fallback/err behavior; no silent NaN/Inf.

---

## ENTRY CONDITIONS (BLOCKING)

- [ ] The need is explicit:
  - either 2+ planned consumers, OR
  - a foundational primitive that is justified as shared infrastructure.
- [ ] The object is measurement-only (computable from data up to time t).
- [ ] The target location under `/v2/crates/math/src/` is identified.
- [ ] A reuse search plan exists (keywords + paths).

---

## EXIT CONDITIONS (DELIVERABLES)

- [ ] A completed module spec document (see "SPEC TEMPLATE", mandatory sections).
- [ ] A written test plan covering determinism, correctness, stability, and performance.
- [ ] A written API + failure contract.
- [ ] A written reuse report (what exists, what is reused, what is new).
- [ ] Ready for ARCHITECT implementation.

---

## REQUIRED READS (BLOCKING)

- `/v2/crates/math/Cargo.toml` (crate boundaries and workspace dependencies)
- `/v2/crates/math/src/lib.rs` (what is exported today)
- `/v2/crates/math/src/bayes/` (reference module layout: docs + tests)
- `/v2/crates/math/src/copula/` (reference module layout: docs + tests + benches log)
- `/v2/crates/algos/docs/protocols/indicators/testing_protocol.md` (testing discipline; math modules must follow the same rigor)

---

## ROLES (SINGLE-AGENT EXECUTION) AND SEPARATION (BLOCKING)

| Role      | Responsibilities                                                                            | Authority                                    |
| --------- | ------------------------------------------------------------------------------------------- | -------------------------------------------- |
| QUANT-PHD | Research, spec, assumptions, test design, performance budget definition                     | REJECT invalid math / predictive framing     |
| ARCHITECT | Implementation + wiring + test code, exactly as specified                                   | REJECT ambiguous spec; request clarification |
| CEO       | Approves new module scope and any tradeoff that impacts determinism/performance/correctness | Final                                        |

**CRITICAL SEPARATION RULE**:

```
PHD: specifies WHAT and WHY
ARCH: writes Rust code (HOW)
PHD: validates by running tests and analyzing results
```

---

# PART A: RESEARCH + REUSE (NO CODE)

## PHASE 0: MODULE INTAKE (MANDATORY)

PHD must define:

- Module name (Rust path + folder name)
- One sentence: "What does it compute/measure?"
- Consumer list (planned 2+ call sites) OR justification why it must be shared anyway
- Output types (what is returned; what is scalar vs vector; what is persisted by callers, if any)

### CHECKPOINT 0: Measurement + Scope (STOP if fail)

- [ ] Measurement-only (no predictive framing)
- [ ] Scope is shared, not indicator-specific glue

---

## PHASE 1: RESEARCH QUESTION + LITERATURE ANCHOR (MANDATORY)

PHD must write:

- Research question in one sentence
- Definition of the mathematical object (not implementation)
- If literature exists: citations (DOI preferred)
- If literature does not exist: explicit statement "No peer-reviewed reference found" (do not invent one)

**Rule**: every non-trivial formula must have either (a) citation, or (b) derivation in the spec.

### 1.1 Approved Research Aids (MCP Tools)

These tools are optional aids for verification. They do not replace proof, citations, or explicit derivations.

- `sympy-mcp`: symbolic derivations and algebraic sanity checks (gradients, likelihood derivatives, constraint algebra).
- `wolfram-alpha`: independent symbolic/numeric cross-check when SymPy is insufficient.
- `serena`: codebase navigation/search to support the reuse report (fast discovery of existing implementations and APIs).
- `context7`: documentation lookup for third-party crates already used in the repo (usage patterns, API details).

**Confidentiality rule (mandatory)**:

- Do not send proprietary datasets, client data, or internal parameter tables to external services.
- If a tool requires an external query (e.g., Wolfram Alpha), only provide generic mathematical expressions or toy examples.

**Evidence rule (mandatory)**:

- If a tool is used to validate a derivation, record the exact input and the resulting conclusion in the spec (short, auditable; no screenshots required).

---

## PHASE 2: ZERO DUPLICATION SEARCH (MANDATORY)

PHD must perform a reuse search across `/v2/crates/math/src/**`.

**Mandatory output**: a "REUSE REPORT" section in the spec with:

- Keywords used
- Paths searched
- Results:
  - candidate file paths (2-10)
  - what each candidate provides
  - decision: REUSE / EXTEND / NOT REUSABLE
  - justification (must be specific, not "different")

**Minimum search** (example; adapt keywords):

```
rg -n "hawkes|self-excit|point process|intensity|ogata|thinning|mle" v2/crates/math/src
rg -n "poisson|exponential kernel|loglik|likelihood|argmin" v2/crates/math/src
```

### CHECKPOINT 0.3: Shared Search Evidence (STOP if fail)

- [ ] Search evidence written
- [ ] All non-reuse decisions justified

---

# PART B: SPECIFICATION (NO CODE)

## SPEC TEMPLATE (MANDATORY SECTIONS)

PHD must create:

`/v2/crates/math/src/[module]/docs/[MODULE_NAME]_SPEC.md`

The spec must include the following sections (mandatory):

1. Identification
2. Purpose and institutional value (as infrastructure)
3. Mathematical specification (notation, algorithm, complexity)
4. Theoretical foundation (citations, assumptions, limitations)
5. Crypto adaptation (where assumptions are violated by crypto microstructure)
6. Determinism contract (explicit)
7. API contract (inputs, outputs, parameters)
8. Failure contract (invalid inputs, non-convergence, fallback)
9. Testing plan (phases below, with counts and acceptance criteria)
10. Performance budget and benchmark plan
11. Reuse report (Phase 2 output)
12. References (DOI preferred)

### CHECKPOINT 1: Spec Completeness (STOP if fail)

- [ ] All sections present
- [ ] Assumptions are testable
- [ ] Determinism + performance budgets are explicit

---

## PHASE 3: DETERMINISM DESIGN (MANDATORY)

The spec MUST answer:

1. Is the algorithm inherently stochastic? If yes: the default path must be reformulated deterministically (REJECT otherwise).
2. Does the algorithm require optimization?
   - deterministic initialization
   - deterministic stopping criteria
   - hard max iterations
   - deterministic tie-breaking and ordering
3. Does parallelism change results? If yes: disable or constrain parallelism in the deterministic path.

### 3.3 Optimizer Policy (MANDATORY, ENFORCED)

Shared modules frequently require optimization (MLE, MAP, constrained parameter fitting). Optimization is a critical production component and MUST follow the optimizer policy below.

**RULE (MANDATORY)**:

- If a shared module requires optimization, the **default deterministic path MUST use a module-specific optimizer** (internal to that module) that is:
  - deterministic (no RNG, no nondeterministic parallel reductions),
  - time-bounded (hard `max_iter`, hard `max_line_search`, explicit stopping criteria),
  - bounded / constrained (explicit parameter bounds and deterministic projection/tie-handling),
  - allocation-stable (no heap allocations inside per-iteration hot loops unless explicitly justified in the spec).

**PROHIBITED (DEFAULT PATH)**:

- Using a generic optimizer borrowed from another shared module (example: importing an optimizer from `shared/garch` or any unrelated module) without explicit CEO approval.
- Any optimizer path that can produce run-to-run variation for the same inputs (Tier C).
- Any optimizer without explicit iteration caps and a defined failure contract.

**ALLOWABLE EXCEPTION (ONLY WITH WRITTEN APPROVAL)**:

- Reusing an existing optimizer is allowed only if:
  - it is already a shared, intentionally generic infrastructure component (not indicator-coupled),
  - the reuse is documented in the **Reuse Report** with a precise justification (no vague “similar”),
  - determinism tier and performance budget remain satisfied,
  - CEO approval is recorded for that reuse decision.

**FAILURE CONTRACT (MANDATORY)**:

- If the optimizer does not converge within `max_iter`, the module MUST return a deterministic error (no silent “best effort” `Ok`).
- The module MUST NOT return `Ok(...)` containing NaN/Inf, even on non-convergence.

### 3.1 Epsilon Determinism Contract (MANDATORY)

MATHILDE shared modules use epsilon-based determinism.

The spec MUST define:

- the equality rule used by tests (absolute and/or relative tolerance),
- the tolerances per output field (or a single tolerance if justified),
- why those tolerances are sufficient (numerical method, scale of outputs, accumulation length).

**Required format**:

```
DETERMINISM (EPSILON-BASED):
- Comparison: abs <= abs_eps OR abs <= rel_eps * max(1, |expected|)
- abs_eps: [value]
- rel_eps: [value]
- Applies to: [list output fields]
- Justification: [why these epsilons are correct for this method and scale]
```

**Hard rule**: tolerance values must not be chosen to "make tests pass"; they must be justified from the method and expected numerical error scale.

---

### 3.2 Determinism Policy: Precision vs Performance (MANDATORY)

Shared modules run under deterministic cron workloads and must remain mathematically correct while meeting time bounds.  
This section defines what "deterministic" means in practice and how precision/performance tradeoffs are allowed.

#### 3.2.1 Determinism tiers (choose exactly one per exported function)

Every exported function MUST explicitly declare one of the following tiers in the spec.

**Tier A: Exact determinism (rare)**

- Meaning: same input -> exactly identical outputs as numbers (not "close").
- Allowed when outputs are discrete or provably exact: integers, counts, indices, ordering outputs, and purely combinatorial structures with deterministic tie-breaking.
- If any float output exists, Tier A is allowed only if exact equality can be proven under the project build/target constraints (do not assume this).

**Tier B: Epsilon determinism (default)**

- Meaning: same input -> outputs equal within the explicit epsilon contract (`abs_eps`, `rel_eps`).
- Required for floating-point outputs unless exact equality is proven.
- The spec MUST state which fields are compared:
  - exactly (integers, lengths, enums, error variants),
  - by epsilon (floats).

**Tier C: Non-deterministic output (REJECT)**

- Any randomness, time-dependent behavior, unordered iteration over hash maps, data races, or parallel reductions that produce run-to-run variation without a deterministic constraint is rejected for shared modules.

#### 3.2.2 "Same input -> same number" rule (MANDATORY)

The spec MUST be explicit about what is promised:

- If Tier A: outputs are identical as numbers (and the proof/argument is written).
- If Tier B: outputs are identical within the epsilon contract, and no stronger claim is made.

**Hard rule**: do not claim exact float equality unless it is proven. Default is Tier B for floats.

#### 3.2.3 Tie-handling and non-uniqueness (MANDATORY)

Some mathematical objects are not unique under ties or degeneracy (examples: MST under equal weights, clustering linkage ties, quantile ties, threshold graphs).  
When non-uniqueness is possible, the implementation MUST:

- define a deterministic tie-break rule (example: sort key `(weight, u, v)`),
- document how ties can change downstream derived measurements (this is identifiability, not numerical error),
- include at least one determinism test that contains ties and proves stable outputs under the tie-break rule.

#### 3.2.4 Parallelism and determinism (MANDATORY)

Parallelism MAY be used only if it preserves the declared determinism tier.

- Default path MUST satisfy the determinism contract.
- Parallel reductions that change floating accumulation order are not allowed in the default deterministic path.
- If a parallel path is offered:
  - it SHOULD be opt-in by config,
  - the spec MUST state whether it remains within the same epsilon contract,
  - determinism tests MUST cover both deterministic path and parallel path.

#### 3.2.5 Optimization and iteration (if applicable) (MANDATORY)

If the module uses fitting/optimization or any iterative method, the spec MUST define:

- deterministic initialization,
- deterministic stopping criteria,
- hard caps (max iterations / max evaluations),
- tolerances (epsilon contract plus optimizer tolerances),
- failure contract for non-convergence (deterministic error or deterministic fallback mode).

## PHASE 4: PERFORMANCE DESIGN (MANDATORY)

The spec MUST define:

- explicit input size bounds used by cron callers (examples: max bars, max events, max dimension)
- a per-call budget in milliseconds for those bounds
- algorithmic complexity and dominant constant factors

**Hard rule**: any iterative method MUST define:

- max iterations
- convergence criteria
- fallback behavior (error or deterministic degraded mode)

### 4.1 Cron Scalability Budget (MANDATORY)

Shared modules are called by cron-driven pipelines. A per-call budget alone is insufficient.

The spec MUST provide:

- `T_call_max_ms`: worst-case per-call budget at explicit input bounds.
- `N_calls_max`: worst-case number of calls per cron run (or per symbol per cron).
- `T_cron_budget_ms = T_call_max_ms * N_calls_max` as the module's worst-case time contribution.

If `T_cron_budget_ms` is material, the design MUST include at least one of:

- algorithmic complexity reduction (preferred, if it preserves correctness),
- allocation reuse via workspace/`*_into(...)` APIs,
- output gating (compute only scalars by default; expensive optional outputs must be opt-in),
- deterministic fast path for repeated calls (must not change the determinism tier).

### CHECKPOINT 2: Feasibility + Budget (STOP if fail)

- [ ] Budget defined and justified
- [ ] Worst-case bounds defined
- [ ] Iteration caps exist where applicable

---

# PART C: IMPLEMENTATION (ARCHITECT)

## PHASE 5: MODULE LAYOUT (MANDATORY)

New module must include:

- `v2/crates/math/src/[module]/mod.rs` (public API surface)
- internal files as needed, but keep public API minimal
- a docs folder produced from the spec path above
- required docs artifacts (must exist even if only placeholders at first):
  - `v2/crates/math/src/[module]/docs/[MODULE_NAME]_SPEC.md`
  - `v2/crates/math/src/[module]/docs/scope.md` (MANDATORY: defines scope + non-scope)
  - `v2/crates/math/src/[module]/docs/[module]_bench_results.md` (append-only)
  - `v2/crates/math/src/[module]/docs/inventory.md`
  - `v2/crates/math/src/[module]/docs/reviews/[module]_module_math_review.md`
- module tests colocated under the module folder:
  - `v2/crates/math/src/[module]/tests/mod.rs` (or `tests/*.rs` with `tests/mod.rs` as entrypoint)
  - `v2/crates/math/src/[module]/mod.rs` MUST declare `#[cfg(test)] mod tests;`

**Test placement rule (mandatory)**:

- All tests MUST live under `v2/crates/math/src/[module]/tests/`.
- Do not add `#[cfg(test)]` test modules inside non-test source files.

Exporting the module requires adding it to:

- `v2/crates/math/src/lib.rs`

**Public API rule**: expose only what consumers need; keep helpers private.

---

## PHASE 6: IMPLEMENTATION RULES (MANDATORY)

ARCH must implement exactly the spec:

- all constants are named and documented
- all divisions/log/sqrt domains are guarded
- no NaN/Inf on `Ok(...)`
- errors are explicit and deterministic
- no hidden mutable global state
- no time-dependent defaults

### 6.0 Code Structure and Comments (MANDATORY)

#### 6.0.1 Minimal Comments (Clarity-Only)

- Comments are allowed only when they increase auditability or prevent misinterpretation.
- Prefer a single-line comment for a non-obvious step inside a function when needed.
- Avoid decorative comments, dividers, or large comment blocks.
- Public API must be self-explanatory via naming; doc comments are allowed for public functions/types when they clarify inputs/outputs, units, constraints, or failure behavior.

#### 6.0.2 Module Decomposition (No Monolith Files)

- Keep `mod.rs` focused on public surface + wiring; put implementations in submodules.
- Do not write huge files that mix unrelated concerns (API, math core, optimization, utilities, tests).
- Decompose by concern (example patterns):
  - `types.rs` (params, result types, errors),
  - `kernel.rs` / `intensity.rs` (core math),
  - `estimation.rs` (fitting/optimization),
  - `likelihood.rs` (objective),
  - `validation.rs` (parameter checks / constraints),
  - `utils.rs` (local-only helpers).
- If a single file becomes large, split it. If not split, the spec must justify why the file must remain unified.

### 6.1 Performance Engineering Rules (MANDATORY)

These rules exist because modules run under deterministic, time-bounded cron workloads.

#### 6.1.1 Allocation and Copy Discipline (HOT PATH)

- No heap allocation inside per-bar/per-event/per-iteration loops unless explicitly justified in the spec.
- No large buffer copying in the critical path (avoid `clone()`, `to_vec()`, `collect()` on large iterators) unless explicitly justified in the spec.
- Prefer slice-based inputs (`&[T]`) and caller-owned buffers where feasible.
- If a function must produce a vector output:
  - pre-allocate deterministically (`Vec::with_capacity`),
  - document the maximum expected size and the source of that bound.

#### 6.1.2 Zero-Copy API Preference

- Prefer taking references to caller-owned data (no internal copies of inputs unless required by correctness).
- If repeated calls are expected, consider an `*_into(out: &mut Vec<_>)` or `*_with_workspace(workspace: &mut ...)` API so allocations can be reused.
- If such an API is not provided, the spec must state why (simplicity, output size small, caller usage not repetitive, etc.).

#### 6.1.3 Parallelism (Rayon) and Determinism

- Default path must satisfy the module determinism contract.
- Parallel reductions that change floating-point aggregation order are not allowed in the default deterministic path.
- If a parallel path is provided, the spec must explicitly define:
  - whether it is opt-in only (recommended), or determinism is guaranteed within the same epsilon contract,
  - how ordering/tie-breaking/reduction is handled,
  - what tests validate determinism under that mode.

### 6.2 Panic-Free + No-`unwrap` Policy (MANDATORY)

Shared modules are production infrastructure. They MUST be panic-free on all inputs covered by the failure contract.

**PROHIBITED in non-test code** (`/src/**` excluding `#[cfg(test)]` modules, and excluding `tests/**`):

- `unwrap()`, `expect(...)`
- `panic!(...)`, `unreachable!()`, `todo!()`
- any float ordering that can panic (example: `partial_cmp(...).unwrap()`), unless finiteness is validated before ordering
- any indexing/conversion that can panic without prior validation (example: `v[i]` without a proven bound, converting negative values to `usize` without checks)

**Allowed only in tests**:

- `unwrap()` / `expect(...)` are allowed in module tests and benchmarks to keep assertions readable.

**MANDATORY error behavior**:

- Any condition that would otherwise cause a panic MUST be converted into a deterministic error variant with a precise failure message.
- Invalid inputs must never cause a panic; they must return a deterministic error variant.

### 6.3 Numerical Safety Contract (MANDATORY)

Every exported function that returns `MathResult<T>` MUST satisfy:

```
NUMERICAL SAFETY:
- If return is Ok(out): all floating values inside `out` are finite (no NaN/Inf).
- If computation cannot produce finite outputs under the declared domains: return Err(...) deterministically.
```

Mandatory guards:

- division: denominator must be finite and non-zero (or handled in an explicitly documented epsilon region)
- `ln`/`log`: argument must be strictly positive
- `sqrt`: argument must be non-negative
- `powf`: guard invalid base/exponent combinations that can produce NaN/Inf
- iterative methods / optimizers: hard caps + deterministic non-convergence error; never `Ok` with “best effort” NaNs

If a mathematically meaningful sentinel is needed (example: “no exceedances”), it MUST be represented explicitly in the API contract (example: `Option<f64>`, or an error), not encoded as NaN inside `Ok(...)`.

### CHECKPOINT 3: Implementation Adherence (STOP if fail)

- [ ] Implementation matches spec (PHD review)
- [ ] Failure modes implemented exactly

---

# PART D: MODULE TESTING (SMALLER THAN INDICATOR TESTING, BUT MANDATORY)

## NO EXPECTATION TWEAKING RULE (MODULE)

When a test fails:

1. FIRST: assume implementation is wrong; fix code.
2. SECOND: only if the math is proven correct, update expectation/tolerance with written proof.
3. THIRD: document changes.

**PROHIBITED**: relaxing tolerances to "make it pass" without proof.

---

## PHASE 7A: DETERMINISM TESTS (MANDATORY) | 3-6 tests

Minimum requirements:

- same input repeated 20 times returns outputs equal within the spec epsilon contract
- repeated run with the same parameters returns identical error variants, not fluctuating outcomes
- order-sensitivity checks for any algorithm that sorts or aggregates

Acceptance:

- determinism statement in spec is satisfied.

---

## PHASE 7B: MATHEMATICAL CORRECTNESS TESTS (MANDATORY) | 4-10 tests

Pick from:

- hand-checkable micro examples
- synthetic data with known parameters (if feasible)
- cross-check against an internal alternative implementation (if one exists)

Acceptance:

- tolerance justified by method and documented (not arbitrary).

### 7B.1 Correctness Evidence Ladder (MANDATORY)

Mathematical correctness for floating-point numerical algorithms cannot be proven for all real inputs using tests alone.
This protocol therefore requires **auditable evidence**: derivation + independent oracles + invariants.

For every non-trivial exported function, tests MUST include at least **two** independent correctness oracles, chosen from:

1. **Closed-form oracle**: compare to a derived closed-form expression on hand-checkable inputs (preferred).
2. **Limit oracle**: verify continuity/limits where the model has a known limit (example: `xi -> 0` branch must match the exponential limit).
3. **Independent implementation oracle**: compare against a second implementation that is not code-shared with the module under test
   (example: a slow reference implementation inside tests; or an older audited module) and justify the independence.
4. **Numerical oracle**:
   - finite-difference check for analytical gradients/Hessians (with step size rationale), OR
   - numerical integration / normalization sanity checks where applicable (explicit bounds and tolerance).

If only one oracle is feasible, the spec MUST explain why, and the test plan MUST compensate with additional invariants and stress cases.

### 7B.2 Optimization Correctness (IF APPLICABLE) | 2-6 tests

If the module exposes an optimizer-driven API (MLE/MAP/iterative solves), tests MUST include:

- **Objective descent**: fitted objective value <= objective at deterministic initialization (within epsilon).
- **Stationarity**: gradient norm at the solution <= configured tolerance (or a justified equivalent stationarity condition).
- **Non-convergence contract**: a constructed hard case that forces hitting `max_iter` returns a deterministic `Err(...)` (no silent `Ok`).
- **Parameter domain enforcement**: solutions respect bounds/constraints (or return error deterministically when impossible).

---

## PHASE 7C: NUMERICAL STABILITY TESTS (MANDATORY) | 6-12 tests

Must include:

- empty / too short inputs (must error deterministically)
- constant inputs
- extreme magnitudes (very small, very large)
- NaN/Inf inputs
- degenerate cases specific to the algorithm (e.g., "no events", "identical timestamps", singular matrix)

Acceptance:

- no NaN/Inf on `Ok`
- explicit errors/fallbacks match the failure contract.

### 7C.1 Finite-Output Assertions (MANDATORY)

Numerical stability tests MUST explicitly assert:

- if a function returns `Ok(out)`, then all float outputs in `out` are finite
- if finiteness cannot be guaranteed under the input, the function returns `Err(...)` (and the error is deterministic)

This is mandatory even when the implementation “looks safe”.

---

## PHASE 7D: PROPERTY / INVARIANT TESTS (MANDATORY) | 3-8 tests

Examples (choose those that apply):

- positivity constraints preserved
- parameter domain constraints preserved
- monotonicity or boundedness where mathematically required
- stationarity constraints enforced or rejected

Acceptance:

- invariants hold OR violations are rejected exactly as specified.

### 7D.1 Metamorphic Tests (MANDATORY WHEN APPLICABLE) | 3-8 tests

For algorithms where exact values are hard to oracle-test across broad inputs, add metamorphic tests:

- ordering/permutation invariance (when mathematically required)
- scaling/translation invariance (when mathematically required and specified)
- monotonicity (example: risk scalar should be non-decreasing in confidence level)
- equivalence between workspace and non-workspace APIs

Metamorphic tests MUST be derived from the mathematical specification; do not invent properties.

---

## PHASE 7E: PERFORMANCE BENCHMARKS (MANDATORY) | 3-6 benchmarks

The module must provide a measurable benchmark plan aligned with:

- existing math module patterns (example references: `v2/crates/math/src/bayes/`, `v2/crates/math/src/copula/`)
- `criterion` is available as a dev dependency (if bench infrastructure is used)
- At least one benchmark case MUST be designed to surface accidental allocations/copies (large-n, repeated calls).
- If the module provides an opt-in parallel path, benchmarks MUST cover deterministic path and parallel path separately.

### 7E.1 Standard benchmark sizes (MANDATORY DEFAULT)

To keep results comparable across modules and over time, benchmarks MUST use the standard size tier set:

- `n = 100` (small)
- `n = 1_000` (medium; aligns with many production/regime workloads and existing copula benches)
- `n = 10_000` (large; upper “routine” bound)

**Rule**: benchmarks MUST NOT exceed `n = 10_000` by default.

Allowed exception (only if justified in the spec and recorded in the bench log entry):

- A module-specific stress size (example: `n = 100_000`) MAY be added as an extra benchmark case when it directly matches an explicit cron bound and the runtime remains acceptable. This is not a replacement for the 100/1k/10k sweep.

### 7E.2 Benchmark coverage (MANDATORY MINIMUM)

The benchmark set MUST include, at minimum:

1. **Scaling sweep**: at least one core hot-path function benchmarked at `n ∈ {100, 1000, 10000}`.
2. **Allocation discipline case**: at least one benchmark that reuses a workspace/`*_into(...)` API (if available) to surface accidental allocations/copies across repeated calls.
3. **Consumer-realistic case**: one benchmark that matches how consumers call the API (example: precompute outside the timed loop vs include preprocessing), and the scope must be stated explicitly in the benchmark name and in the log.

Acceptance:

- meets the declared budget for declared bounds, OR the design is rejected/reworked.

---

## PHASE 7F: PANIC-SAFETY TESTS (MANDATORY) | 2-6 tests

Shared modules MUST NOT panic in production code paths.

Minimum requirements:

- each exported function is exercised at least once under:
  - a valid nominal case, and
  - an invalid-input case (as declared by the failure contract),
    and the test asserts **no panic** (return is `Ok` or `Err`, but never a panic).

Acceptance:

- no panics triggered by invalid inputs (including NaN/Inf, empty inputs, domain violations).

---

## PHASE 7G: FAILURE-CONTRACT COVERAGE TESTS (MANDATORY) | 3-10 tests

The failure contract is part of mathematical correctness.

Minimum requirements:

- for each documented failure condition in the spec, include at least one test that triggers it
- the test MUST assert:
  - the function returns `Err(...)` (not `Ok` with sentinel NaN/Inf), and
  - the error variant is deterministic and stable (no changing variants across runs)

Acceptance:

- all documented failure conditions are covered by tests, or the spec is incomplete (STOP).

---

## PHASE 7H: BENCHMARK LOGGING (MANDATORY) | ON EVERY PERF-RELEVANT CHANGE

Benchmarks are not a one-time checkbox. They are part of the production performance contract.

**Requirement**: every shared module MUST maintain a benchmark results log under:

- `v2/crates/math/src/[module]/docs/[module]_bench_results.md`

Policy (mandatory):

- The file is **append-only** (never rewrite history; add a new entry per run).
- Every perf-relevant change (algorithm, loops, allocation behavior, workspace API, optimizer changes) MUST be followed by a benchmark run and a new log entry.
- The entry MUST record at minimum:
  - date (UTC), operator name/role,
  - command executed (exact),
  - build profile (debug/release),
  - machine info (CPU model, core count, RAM) and OS,
  - key benchmark results (median + dispersion) for the declared bounds (including the `n` sizes used; default is `n ∈ {100, 1000, 10000}`),
  - any regressions vs previous entry (explicitly stated).

Acceptance:

- benchmark log exists and contains a complete history of runs relevant to the current module version.

---

## PHASE 7I: DOCUMENTATION ARTIFACTS (MANDATORY) | AFTER IMPLEMENTATION IS STABLE

When the module is functionally complete and tests/benches are passing, ARCH MUST produce two discovery/audit documents:

1. **Mathematical Review (mandatory)**:

   - Path: `v2/crates/math/src/[module]/docs/reviews/[module]_module_math_review.md`
   - Template requirement: follow the structure of an existing approved module review (example: `v2/crates/math/src/bayes/docs/reviews/bayes_module_math_review.md`).
   - Content requirement: bind every non-trivial formula/claim to the exact implementation file and line references, and point to the specific test(s) that validate it.

2. **Inventory (mandatory)**:
   - Path: `v2/crates/math/src/[module]/docs/inventory.md`
   - Content requirement:
     - list all source files under the module and the role of each file (1 sentence per file),
     - list the public API surface (functions/types) and what each computes (1 sentence each),
     - list the workspace/`*_into` APIs (if present) and what allocations they avoid,
     - list optional diagnostics and their runtime category (cheap vs expensive),
     - list determinism tier (A/B) and epsilon policy reference.

Acceptance:

- both documents exist and are consistent with the actual code.

---

## TEST COMMANDS (REFERENCE)

Run tests by package and by module filter:

- Run math crate tests: `cargo test -p math`
- Run module tests (filter): `cargo test -p math [module]::`
- Run with output: `cargo test -p math [module]:: -- --nocapture`

Benchmarks (if present):

- `cargo bench -p math`

---

# PART E: READY GATE + HANDOFF

### CHECKPOINT 4: Ready for Consumers (BLOCKING)

- [ ] Spec exists at the module docs path
- [ ] Reuse report exists and is defensible
- [ ] Determinism tests pass
- [ ] Correctness tests pass
- [ ] Stability tests pass
- [ ] Performance benchmarks meet budget (or CEO-approved exception with documented rationale)
- [ ] Benchmark results log exists and is up to date (`docs/[module]_bench_results.md`)
- [ ] Mathematical review exists (`docs/reviews/[module]_module_math_review.md`)
- [ ] Inventory exists (`docs/inventory.md`)
- [ ] Scope document exists (`docs/scope.md`)
- [ ] Public API is minimal and documented

**FAIL = STOP. DO NOT INTEGRATE INTO INDICATORS.**

---

## KNOWN FAILURE MODES (REFERENCE)

- Duplication: re-implemented stats/optimizer already exists in `v2/crates/math/src/`
- Non-determinism: floating-point accumulation order changes with parallelism
- Unbounded runtime: iterative method without max iteration or without budget
- Silent NaN: returning `Ok` with NaN/Inf under degenerate windows
- Panic: `unwrap/expect` or unchecked float ordering causes runtime panic on invalid inputs
- Hidden defaults: parameters implied by code instead of explicit config

---

**END PROTOCOL v1.0**
