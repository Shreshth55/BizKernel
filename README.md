# BizKernel

An agentic ERP in Rust. Business users define and run their own operational logic in plain language — no developer or ABAPer in the loop. An agent turns intent into typed operations; the Rust kernel is the sole writer and refuses anything that would break integrity.

```
Business user ──intent──▶ Agent ──operations──▶ Kernel (Rust) ──▶ Hash-chained event store
                            ▲                     │
                            └──rejection + reason─┘
```

**Agent proposes, the kernel disposes.** The agent has no path to storage except through the kernel.

## Workspace

| Crate | Role |
| --- | --- |
| `bizkernel-store` | `Storage` trait + append-only, SHA-256 hash-chained event store. Detects edits, deletions and re-sealed forgeries. |
| `bizkernel-core` | Typed procure-to-pay domain and the `Kernel`: validates every `Operation`, seals it into the chain, applies it. State rebuilds from the chain. |
| `bizkernel-agent` | Tool surface for an LLM: JSON tool schemas + `dispatch`, which routes every call through the kernel. |

## Integrity rules (procure-to-pay)

- A PO needs at least one line; quantities and prices must be positive.
- Goods can only be received against an **approved** PO, and never beyond the ordered quantity.
- Invoices are **3-way matched**: quantity ≤ received-and-not-yet-invoiced, amount = qty × PO price.
- A PO with goods received cannot be cancelled.
- A rejected operation writes nothing.
- A tampered chain refuses to open.

## Build

```sh
cargo test
```

## Roadmap

0. Define the slice — procure-to-pay entities and rules ✅
1. Rust transactional core — typed domain, validated operations, hash-chained store ← **in progress**
2. Agent layer — LLM loop on top of the tool surface
3. Business-defined artifacts — custom reports, then fields, then workflows
4. Crypto anchor — publish the chain head for external verification
5. Custom storage engine behind the `Storage` trait

## License

Proprietary. © Shreshth. All rights reserved.
