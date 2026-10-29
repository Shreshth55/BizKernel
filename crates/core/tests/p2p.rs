use bizkernel_core::*;
use bizkernel_store::{MemoryStore, Storage};

fn approved_po(k: &mut Kernel<MemoryStore>) -> PoId {
    let created = k
        .submit(Operation::CreatePo {
            vendor: VendorId(100),
            lines: vec![
                LineSpec { material: MaterialId(1), qty: 10, unit_price: 500 },
                LineSpec { material: MaterialId(2), qty: 4, unit_price: 2_000 },
            ],
        })
        .unwrap();
    let po = created.assigned.unwrap();
    k.submit(Operation::ApprovePo { po }).unwrap();
    po
}

fn kernel() -> Kernel<MemoryStore> {
    Kernel::open(MemoryStore::new()).unwrap()
}

#[test]
fn full_procure_to_pay_flow() {
    let mut k = kernel();
    let po = approved_po(&mut k);
    k.submit(Operation::PostGoodsReceipt { po, line: 1, qty: 8 }).unwrap();
    k.submit(Operation::PostInvoice { po, line: 1, qty: 8, amount: 4_000 }).unwrap();

    let line = k.po(po).unwrap().line(1).unwrap();
    assert_eq!((line.qty_received, line.qty_invoiced), (8, 8));
    assert_eq!(k.po(po).unwrap().total_value(), 13_000);
    assert_eq!(k.store().records().len(), 4);
}

#[test]
fn goods_receipt_cannot_exceed_po() {
    let mut k = kernel();
    let po = approved_po(&mut k);
    k.submit(Operation::PostGoodsReceipt { po, line: 1, qty: 8 }).unwrap();
    let err = k.submit(Operation::PostGoodsReceipt { po, line: 1, qty: 3 }).unwrap_err();
    assert_eq!(err, Rejection::OverReceipt { po, line: 1, qty: 3, open: 2 });
    // A rejected operation writes nothing.
    assert_eq!(k.store().records().len(), 3);
}

#[test]
fn cannot_receive_against_unapproved_po() {
    let mut k = kernel();
    let po = k
        .submit(Operation::CreatePo {
            vendor: VendorId(1),
            lines: vec![LineSpec { material: MaterialId(1), qty: 1, unit_price: 1 }],
        })
        .unwrap()
        .assigned
        .unwrap();
    let err = k.submit(Operation::PostGoodsReceipt { po, line: 1, qty: 1 }).unwrap_err();
    assert!(matches!(err, Rejection::WrongStatus { .. }));
}

#[test]
fn three_way_match_enforced() {
    let mut k = kernel();
    let po = approved_po(&mut k);
    k.submit(Operation::PostGoodsReceipt { po, line: 2, qty: 2 }).unwrap();

    let over = k.submit(Operation::PostInvoice { po, line: 2, qty: 3, amount: 6_000 }).unwrap_err();
    assert!(matches!(over, Rejection::InvoiceExceedsReceipt { open: 2, .. }));

    let price = k.submit(Operation::PostInvoice { po, line: 2, qty: 2, amount: 4_500 }).unwrap_err();
    assert!(matches!(price, Rejection::PriceMismatch { expected: 4_000, .. }));
    println!("{price}");
}

#[test]
fn cannot_cancel_after_receipt() {
    let mut k = kernel();
    let po = approved_po(&mut k);
    k.submit(Operation::PostGoodsReceipt { po, line: 1, qty: 1 }).unwrap();
    assert_eq!(k.submit(Operation::CancelPo { po }).unwrap_err(), Rejection::HasReceipts { po });
}

#[test]
fn bad_po_lines_rejected() {
    let mut k = kernel();
    assert_eq!(k.submit(Operation::CreatePo { vendor: VendorId(1), lines: vec![] }).unwrap_err(), Rejection::NoLines);
    let zero = k
        .submit(Operation::CreatePo {
            vendor: VendorId(1),
            lines: vec![LineSpec { material: MaterialId(1), qty: 0, unit_price: 5 }],
        })
        .unwrap_err();
    assert_eq!(zero, Rejection::ZeroQty { line: 1 });
}

#[test]
fn state_rebuilds_from_the_chain() {
    let mut k = kernel();
    let po = approved_po(&mut k);
    k.submit(Operation::PostGoodsReceipt { po, line: 1, qty: 5 }).unwrap();
    let expected = k.po(po).unwrap().clone();

    let mut store = MemoryStore::new();
    for r in k.store().records() {
        store.append(r.payload.clone());
    }
    let replayed = Kernel::open(store).unwrap();
    assert_eq!(replayed.po(po).unwrap(), &expected);
}

#[test]
fn tampered_chain_refuses_to_open() {
    let mut k = kernel();
    let po = approved_po(&mut k);
    k.submit(Operation::PostGoodsReceipt { po, line: 1, qty: 5 }).unwrap();

    let mut store = MemoryStore::new();
    for r in k.store().records() {
        store.append(r.payload.clone());
    }
    let forged = String::from_utf8(store.records()[2].payload.clone()).unwrap().replace("5", "9");
    store.records_mut()[2].payload = forged.into_bytes();
    assert!(matches!(Kernel::open(store), Err(ReplayError::Chain(_))));
}
