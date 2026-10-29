//! Operations: the only way state can change. The agent proposes these;
//! the kernel accepts or rejects them.

use crate::domain::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Operation {
    CreatePo { vendor: VendorId, lines: Vec<LineSpec> },
    ApprovePo { po: PoId },
    CancelPo { po: PoId },
    PostGoodsReceipt { po: PoId, line: u32, qty: Qty },
    PostInvoice { po: PoId, line: u32, qty: Qty, amount: Amount },
}

/// Why the kernel refused an operation. Every message is written to be read
/// back to a business user as-is.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Rejection {
    #[error("a purchase order needs at least one line")]
    NoLines,
    #[error("line {line}: quantity must be greater than zero")]
    ZeroQty { line: u32 },
    #[error("line {line}: unit price must be greater than zero")]
    ZeroPrice { line: u32 },
    #[error("{0} does not exist")]
    UnknownPo(PoId),
    #[error("{po} has no line {line}")]
    UnknownLine { po: PoId, line: u32 },
    #[error("{po} is {status:?}; this needs it to be {needed:?}")]
    WrongStatus { po: PoId, status: PoStatus, needed: PoStatus },
    #[error("{po} already has goods received, so it can't be cancelled")]
    HasReceipts { po: PoId },
    #[error("{po} line {line}: receiving {qty} would exceed the ordered quantity ({open} still open)")]
    OverReceipt { po: PoId, line: u32, qty: Qty, open: Qty },
    #[error("{po} line {line}: invoicing {qty} exceeds what has been received and not yet invoiced ({open})")]
    InvoiceExceedsReceipt { po: PoId, line: u32, qty: Qty, open: Qty },
    #[error("{po} line {line}: invoice amount {amount} doesn't match {qty} x PO price {unit_price} = {expected}")]
    PriceMismatch { po: PoId, line: u32, qty: Qty, amount: Amount, unit_price: Amount, expected: Amount },
    #[error("amount overflow")]
    Overflow,
}

/// What an accepted operation produced; this is what gets written to the chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub op: Operation,
    /// Set for `CreatePo`: the id the kernel assigned.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub assigned: Option<PoId>,
}
