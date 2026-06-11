# Token Fundraiser — v1: Raw Pinocchio Baseline

> **Series:** Solana CU Optimization · Part 1 of 3

A fundraising program for SPL Tokens built on Solana using the
[Pinocchio](https://github.com/anza-xyz/pinocchio) framework. This version
is the **unoptimized baseline** — written following standard Pinocchio
patterns with no manual CU tuning. It exists to establish a honest
performance floor and to make every optimization in v2 and v3 legible
by contrast.

---

## What It Does

A maker creates a fundraiser specifying:
- The SPL mint token they want to collect
- A target amount to raise
- A duration in days

Contributors can send tokens into a program-owned vault. Once the target
is met the maker can claim the vault. If the deadline passes without
reaching the target, contributors can claim refunds.

---

## Instructions

| Instruction   | Accounts | Description |
|---------------|----------|-------------|
| `Initialize`  | 7        | Create the fundraiser account and vault ATA |
| `Contribute`  | 9        | Deposit tokens; create contributor PDA on first contribution |
| `Checker`     | 7        | Verify goal reached and transfer vault to maker |
| `Refund`      | 9        | Return contributor tokens after deadline with unmet goal |

---

## How to Run

```bash
# Build the program
cargo build-sbf

# Run all tests with output
cargo test -- --nocapture
```

---

## Optimization Approach

**None applied.** This version intentionally uses:

- Standard `#[derive(Clone, Copy, Debug, PartialEq)]` on all state structs
- `u64::from_le_bytes(slice.try_into().unwrap())` for all integer reads
- `copy_from_slice` for all 32-byte key writes
- `pinocchio_token::state::Account::from_account_view()` for token account parsing
- `Rent::get()?` sysvar calls inside `CreateAccount`
- `Clock::get()?` sysvar calls for timestamp reads
- `derive_address` for PDA verification before every CPI
- Full Pinocchio safety model throughout

This is the code pattern most developers write on their first pass with
Pinocchio. No Cargo profile tuning, no unsafe blocks, no hardcoded constants.

---

## Test Results

```
running 7 tests

Final Binary Size: 52.44 KB
test test::tests::test_binary_size ... FAILED   ← exceeds L1 iCache threshold

Initialize CU:  16,628   (with new ATA creation)
Initialize CU:  16,628   (second run, ATA exists)
Initialize CU:  18,128   (with extra account overhead)
Contribute CU:   3,209
Contribute CU:   3,161
Refund CU:       1,870
Checker CU:      1,341

Estimated CU per account parsed: 2,311

test result: FAILED. 6 passed; 1 failed
```

---

## Performance Summary

| Metric              | Value     | Notes |
|---------------------|-----------|-------|
| Binary size         | 52.44 KB  | ❌ Fails L1 iCache test (threshold: ~32 KB) |
| Initialize CU       | 16,628    | ATA creation dominates |
| Contribute CU       | 3,209     | Highest across all versions |
| Refund CU           | 1,870     | Highest across all versions |
| Checker CU          | 1,341     | Highest across all versions |
| Tests passing       | 6 / 7     | Binary size test fails |

---

## Where the CUs Go

The dominant cost sources in this version, in order:

1. **`CreateATA` CPI** — ~14,000 CU inside `Initialize`. Two nested CPIs
   (CreateAccount + InitializeAccount) called by the ATA program. Irreducible.
2. **`derive_address` syscall** — ~1,500 CU per call. Called in `Initialize`,
   `Contribute`, and `Refund` before every `invoke_signed`. Completely avoidable.
3. **`Rent::get()?`** — ~100 CU sysvar syscall inside each `CreateAccount`.
   Can be replaced with a hardcoded constant.
4. **`Clock::get()?`** — ~150–200 CU sysvar syscall. Called in `Contribute`
   and `Refund`. Can be replaced by passing the Clock account.
5. **`TokenAccountState::from_account_view()`** — ~50 CU per call. Adds
   validation overhead for a layout that is stable and well-known.
6. **`from_le_bytes().try_into().unwrap()`** — ~15–30 CU per integer read
   instead of a single `ldxdw` instruction (1 CU).
7. **Binary bloat from derives** — `#[derive(Debug, Clone, Copy, PartialEq)]`
   on every struct. Debug alone pulls in significant formatting code.
   52 KB of binary stresses instruction cache, adding ~5–15% overhead
   across all non-CPI instructions.

---

## Pros

- **Maximum readability** — any Pinocchio developer can read and audit
  this code immediately with no specialized knowledge
- **Zero undefined behaviour** — all operations go through Pinocchio's
  validated wrappers
- **Easy to onboard contributors** — follows the same patterns as official
  Pinocchio examples
- **Honest baseline** — because no micro-optimizations are applied, any
  regression introduced by v2 or v3 techniques is immediately visible
- **Simplest debugging** — `#[derive(Debug)]` means you can print every
  state struct during development without extra code
- **Standard error handling** — `?` propagation and `Result` types make
  failure paths easy to follow

---

## Cons

- **Binary size: 52.44 KB** — fails the L1 instruction cache optimization
  test. When the binary exceeds the processor's L1 iCache, the runtime
  pays cache-miss penalties on every instruction fetch, degrading CU
  efficiency across the entire program
- **Highest CU usage** — Contribute costs 521 CU more than v2 (16% worse);
  Refund costs 288 CU more (15% worse); Checker costs 90 CU more (7% worse)
- **Three wasted `derive_address` calls** — ~4,500 CU per full transaction
  flow spent on SHA-256 hashing that the runtime performs anyway during
  `invoke_signed` validation
- **Sysvar syscalls on every time-sensitive instruction** — `Rent::get()`
  and `Clock::get()` add ~250–300 CU of unavoidable syscall overhead per
  invocation when hardcoded or account-passed alternatives exist
- **`try_into().unwrap()` on slice reads** — every integer deserialization
  generates bounds-check and panic branch infrastructure that inflates
  the binary and costs extra instructions
- **Not suitable for CU-competitive environments** — at 3,209 CU for
  Contribute, this version would be meaningfully more expensive to use
  at scale than the optimized alternatives

---

## Comparison Baseline

| Instruction | v1 (this) | v2 optimized | v3 unsafe | Best possible |
|-------------|-----------|--------------|-----------|---------------|
| Initialize  | 16,628 CU | 16,174 CU    | 16,179 CU | ~15,800 CU†   |
| Contribute  |  3,209 CU |  2,688 CU    |  2,612 CU | ~2,500 CU†    |
| Refund      |  1,870 CU |  1,582 CU    |  1,493 CU | ~1,400 CU†    |
| Checker     |  1,341 CU |  1,251 CU    |  1,268 CU | ~1,200 CU†    |
| Binary size |  52.44 KB |    14.20 KB  |   15.59 KB| ~12 KB†        |

_† Theoretical minimum: CPIs + mandatory syscalls, no code overhead_

---

## When to Use This Version

- Learning Pinocchio patterns for the first time
- As the starting point before applying optimization passes
- When auditability and developer ergonomics matter more than CU cost
- In development/testing environments where CU usage is not measured

> For any production deployment or CU-competitive context,
> use **v2** (optimized) or **v3** (unsafe) instead.
