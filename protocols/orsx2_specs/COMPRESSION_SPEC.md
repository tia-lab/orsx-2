# ORSX2 — Compression Spec (TEMPLATE)

Status: DRAFT  
Owner: (Designer)  
Applies to: `orsx2::types::Compressed<T>`

## 1) Identification

- Purpose: transparent storage of large numeric vectors in Postgres `BYTEA`.
- Non-scope: general-purpose compression for arbitrary structs.

## 2) Supported element types

- Integers:
- Floats:
- Maximum length:

## 3) Envelope format (mandatory, versioned)

Define the exact binary layout:

- Magic:
- Version:
- Codec id:
- Element type id:
- Element count:
- Uncompressed byte length:
- Checksum algorithm and coverage:
- Payload:

## 4) Compatibility policy

- Backward compatibility:
- Forward compatibility:
- Unknown version behavior:

## 5) Failure contract

- Invalid header:
- Unsupported element type:
- Checksum mismatch:
- Codec decode failure:

All must return deterministic errors and must not panic.

## 6) Performance budgets

- Encode throughput target (MB/s) at representative sizes:
- Decode throughput target (MB/s) at representative sizes:
- Allocation policy (caller-owned buffers?):

## 6.1 Workspace / zero-copy API plan (mandatory)

Define:

- `compress_into(out: &mut Vec<u8>, ...)` behavior (reuse vs grow)
- `decompress_into(out: &mut Vec<T>, ...)` behavior (reuse vs grow)
- workspace type(s) (if codec needs scratch buffers)

## 7) Test plan mapping

- Round-trip property tests:
- Invalid payload tests:
- DB encode/decode integration tests:
- Benchmarks:
