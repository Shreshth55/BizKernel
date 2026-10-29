# BizKernel

An agentic ERP in Rust. Business users define and run their own operational logic in plain language — no developer or ABAPer in the loop. An agent turns intent into typed operations; the Rust kernel is the sole writer and refuses anything that would break integrity.

```
Business user ──intent──▶ Agent ──operations──▶ Kernel (Rust) ──▶ Hash-chained event store
                            ▲                     │
                            └──rejection + reason─┘
```

**Agent proposes, the kernel disposes.** The agent has no path to storage except through the kernel.

## Why BizKernel beats SAP

SAP's real cost isn't its database. It's the screens, the T-codes, the consultants, and the developer queue between a business need and a working system. BizKernel removes all of that.

| | SAP and legacy ERPs | BizKernel |
| --- | --- | --- |
| **Interface** | Dense GUI screens, hundreds of T-codes, weeks of training | Conversation-first: type or say what you need. Clean, minimal screens shaped around the task in front of you. No training. |
| **Custom logic** | Ticket → ABAPer → test → transport. Weeks. | Describe it in plain language. Live in seconds. |
| **Who's in control** | IT and consultants gatekeep every change | The business owns its own system |
| **Correctness** | Enforced by convention and human review | Enforced by Rust types and kernel rules. Invalid data cannot be written. |
| **Audit** | Mutable logs; "trust us" | Every change hash-chained and tamper-evident; verifiable by anyone |
| **Speed to value** | Months-long implementations | Running the same day |
| **Fit** | Built for the largest enterprises, priced for them | Lean enough for a 50-person business, solid enough to scale |

### The interface is the product

Most ERPs bolt a chat window onto old screens. BizKernel starts from the other end: the conversation *is* the interface, and screens appear only when they help.

- **Ask, don't navigate.** "Show open POs from Vendor 100 over ₹5L" returns the answer, not a menu path.
- **Every action explains itself.** When the kernel refuses something, you get the reason in plain words and what would fix it.
- **One clean surface for every role.** Buyers, store managers and finance teams use the same simple interface, each seeing only what their work needs.
- **Fast by default.** A Rust core with no middleware stack between you and your data.

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
4. Conversation-first UI — chat plus task-shaped screens for every role
5. Crypto anchor — publish the chain head for external verification
6. Custom storage engine behind the `Storage` trait

## License

Proprietary. © Shreshth. All rights reserved.
