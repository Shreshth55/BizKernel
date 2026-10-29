//! Procure-to-pay domain types. Quantities and amounts are unsigned integers
//! (amounts in minor units, e.g. paise), so negative stock or money can't exist.

use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! id {
    ($name:ident, $prefix:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub u64);

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!($prefix, "{}"), self.0)
            }
        }
    };
}

id!(PoId, "PO-");
id!(VendorId, "V-");
id!(MaterialId, "M-");

/// Whole units of a material.
pub type Qty = u64;
/// Money in minor units (paise / cents).
pub type Amount = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PoStatus {
    Draft,
    Approved,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineSpec {
    pub material: MaterialId,
    pub qty: Qty,
    pub unit_price: Amount,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoLine {
    pub material: MaterialId,
    pub qty_ordered: Qty,
    pub unit_price: Amount,
    pub qty_received: Qty,
    pub qty_invoiced: Qty,
}

impl PoLine {
    pub fn open_to_receive(&self) -> Qty {
        self.qty_ordered - self.qty_received
    }

    pub fn open_to_invoice(&self) -> Qty {
        self.qty_received - self.qty_invoiced
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurchaseOrder {
    pub id: PoId,
    pub vendor: VendorId,
    pub status: PoStatus,
    /// Indexed from 1 in operations, like SAP item numbers.
    pub lines: Vec<PoLine>,
}

impl PurchaseOrder {
    pub fn line(&self, line_no: u32) -> Option<&PoLine> {
        self.lines.get((line_no as usize).checked_sub(1)?)
    }

    pub fn line_mut(&mut self, line_no: u32) -> Option<&mut PoLine> {
        self.lines.get_mut((line_no as usize).checked_sub(1)?)
    }

    pub fn total_value(&self) -> Amount {
        self.lines.iter().map(|l| l.qty_ordered * l.unit_price).sum()
    }
}
