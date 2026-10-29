//! The kernel: validates an operation against current state, seals the
//! resulting event into the hash chain, then applies it. Nothing is written
//! unless every rule passes, and state can always be rebuilt from the chain.

use crate::domain::*;
use crate::ops::*;
use bizkernel_store::{ChainError, Storage};
use std::collections::BTreeMap;

#[derive(Debug, thiserror::Error)]
pub enum ReplayError {
    #[error(transparent)]
    Chain(#[from] ChainError),
    #[error("record {seq}: unreadable event: {source}")]
    Decode { seq: u64, source: serde_json::Error },
    #[error("record {seq}: event no longer passes the rules: {rejection}")]
    Invalid { seq: u64, rejection: Rejection },
}

pub struct Kernel<S: Storage> {
    store: S,
    pos: BTreeMap<PoId, PurchaseOrder>,
    next_po: u64,
}

impl<S: Storage> Kernel<S> {
    /// Rebuild state from an existing chain, verifying it first.
    pub fn open(store: S) -> Result<Self, ReplayError> {
        store.verify()?;
        let mut k = Kernel { store, pos: BTreeMap::new(), next_po: 4_500_000_001 };
        let events: Vec<(u64, Event)> = k
            .store
            .records()
            .iter()
            .map(|r| {
                serde_json::from_slice(&r.payload)
                    .map(|e| (r.seq, e))
                    .map_err(|source| ReplayError::Decode { seq: r.seq, source })
            })
            .collect::<Result<_, _>>()?;
        for (seq, event) in events {
            k.check(&event.op).map_err(|rejection| ReplayError::Invalid { seq, rejection })?;
            k.apply(&event);
        }
        Ok(k)
    }

    /// Validate, record, apply. The only write path.
    pub fn submit(&mut self, op: Operation) -> Result<Event, Rejection> {
        self.check(&op)?;
        let assigned = matches!(op, Operation::CreatePo { .. }).then_some(PoId(self.next_po));
        let event = Event { op, assigned };
        let payload = serde_json::to_vec(&event).expect("events always serialize");
        self.store.append(payload);
        self.apply(&event);
        Ok(event)
    }

    pub fn po(&self, id: PoId) -> Option<&PurchaseOrder> {
        self.pos.get(&id)
    }

    pub fn purchase_orders(&self) -> impl Iterator<Item = &PurchaseOrder> {
        self.pos.values()
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    fn get(&self, po: PoId) -> Result<&PurchaseOrder, Rejection> {
        self.pos.get(&po).ok_or(Rejection::UnknownPo(po))
    }

    fn require(order: &PurchaseOrder, needed: PoStatus) -> Result<(), Rejection> {
        if order.status == needed {
            Ok(())
        } else {
            Err(Rejection::WrongStatus { po: order.id, status: order.status, needed })
        }
    }

    fn get_line(order: &PurchaseOrder, line: u32) -> Result<&PoLine, Rejection> {
        order.line(line).ok_or(Rejection::UnknownLine { po: order.id, line })
    }

    /// Every integrity rule lives here. Pure: reads state, changes nothing.
    fn check(&self, op: &Operation) -> Result<(), Rejection> {
        match op {
            Operation::CreatePo { lines, .. } => {
                if lines.is_empty() {
                    return Err(Rejection::NoLines);
                }
                for (i, l) in lines.iter().enumerate() {
                    let line = i as u32 + 1;
                    if l.qty == 0 {
                        return Err(Rejection::ZeroQty { line });
                    }
                    if l.unit_price == 0 {
                        return Err(Rejection::ZeroPrice { line });
                    }
                    l.qty.checked_mul(l.unit_price).ok_or(Rejection::Overflow)?;
                }
                Ok(())
            }
            Operation::ApprovePo { po } => Self::require(self.get(*po)?, PoStatus::Draft),
            Operation::CancelPo { po } => {
                let order = self.get(*po)?;
                if order.status == PoStatus::Cancelled {
                    return Err(Rejection::WrongStatus { po: *po, status: order.status, needed: PoStatus::Approved });
                }
                if order.lines.iter().any(|l| l.qty_received > 0) {
                    return Err(Rejection::HasReceipts { po: *po });
                }
                Ok(())
            }
            Operation::PostGoodsReceipt { po, line, qty } => {
                let order = self.get(*po)?;
                Self::require(order, PoStatus::Approved)?;
                let l = Self::get_line(order, *line)?;
                if *qty == 0 {
                    return Err(Rejection::ZeroQty { line: *line });
                }
                let open = l.open_to_receive();
                if *qty > open {
                    return Err(Rejection::OverReceipt { po: *po, line: *line, qty: *qty, open });
                }
                Ok(())
            }
            Operation::PostInvoice { po, line, qty, amount } => {
                let order = self.get(*po)?;
                Self::require(order, PoStatus::Approved)?;
                let l = Self::get_line(order, *line)?;
                if *qty == 0 {
                    return Err(Rejection::ZeroQty { line: *line });
                }
                let open = l.open_to_invoice();
                if *qty > open {
                    return Err(Rejection::InvoiceExceedsReceipt { po: *po, line: *line, qty: *qty, open });
                }
                let expected = qty.checked_mul(l.unit_price).ok_or(Rejection::Overflow)?;
                if *amount != expected {
                    return Err(Rejection::PriceMismatch {
                        po: *po,
                        line: *line,
                        qty: *qty,
                        amount: *amount,
                        unit_price: l.unit_price,
                        expected,
                    });
                }
                Ok(())
            }
        }
    }

    /// Apply an already-checked event. Infallible by construction.
    fn apply(&mut self, event: &Event) {
        match &event.op {
            Operation::CreatePo { vendor, lines } => {
                let id = event.assigned.expect("CreatePo events carry their id");
                let order = PurchaseOrder {
                    id,
                    vendor: *vendor,
                    status: PoStatus::Draft,
                    lines: lines
                        .iter()
                        .map(|s| PoLine {
                            material: s.material,
                            qty_ordered: s.qty,
                            unit_price: s.unit_price,
                            qty_received: 0,
                            qty_invoiced: 0,
                        })
                        .collect(),
                };
                self.pos.insert(id, order);
                self.next_po = self.next_po.max(id.0 + 1);
            }
            Operation::ApprovePo { po } => self.order_mut(*po).status = PoStatus::Approved,
            Operation::CancelPo { po } => self.order_mut(*po).status = PoStatus::Cancelled,
            Operation::PostGoodsReceipt { po, line, qty } => {
                self.line_mut(*po, *line).qty_received += qty;
            }
            Operation::PostInvoice { po, line, qty, .. } => {
                self.line_mut(*po, *line).qty_invoiced += qty;
            }
        }
    }

    fn order_mut(&mut self, po: PoId) -> &mut PurchaseOrder {
        self.pos.get_mut(&po).expect("checked")
    }

    fn line_mut(&mut self, po: PoId, line: u32) -> &mut PoLine {
        self.order_mut(po).line_mut(line).expect("checked")
    }
}
