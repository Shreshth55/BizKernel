//! The agent's entire power: call these tools. Each call is parsed into a
//! typed `Operation` and handed to the kernel, which accepts or rejects it.
//! There is no path from here to storage that bypasses the kernel.
//!
//! The LLM loop itself (intent -> tool calls) plugs in on top of `tools()`
//! and `dispatch()`; it is provider-agnostic and not part of this crate yet.

use bizkernel_core::{Kernel, Operation};
use bizkernel_store::Storage;
use serde_json::{json, Value};

/// JSON tool definitions in the common function-calling shape.
pub fn tools() -> Value {
    let po = json!({"type": "integer", "description": "Purchase order number"});
    let line = json!({"type": "integer", "minimum": 1, "description": "PO item number, from 1"});
    let qty = json!({"type": "integer", "minimum": 1});
    let amount = json!({"type": "integer", "minimum": 1, "description": "Minor units (paise)"});
    json!([
        {
            "name": "create_po",
            "description": "Create a draft purchase order for a vendor.",
            "input_schema": {
                "type": "object",
                "required": ["vendor", "lines"],
                "properties": {
                    "vendor": {"type": "integer"},
                    "lines": {"type": "array", "minItems": 1, "items": {
                        "type": "object",
                        "required": ["material", "qty", "unit_price"],
                        "properties": {"material": {"type": "integer"}, "qty": qty, "unit_price": amount}
                    }}
                }
            }
        },
        {"name": "approve_po", "description": "Approve a draft purchase order.",
         "input_schema": {"type": "object", "required": ["po"], "properties": {"po": po}}},
        {"name": "cancel_po", "description": "Cancel a purchase order with no goods received.",
         "input_schema": {"type": "object", "required": ["po"], "properties": {"po": po}}},
        {"name": "post_goods_receipt", "description": "Receive goods against an approved PO line.",
         "input_schema": {"type": "object", "required": ["po", "line", "qty"],
                          "properties": {"po": po, "line": line, "qty": qty}}},
        {"name": "post_invoice", "description": "Post a vendor invoice against received goods (3-way match).",
         "input_schema": {"type": "object", "required": ["po", "line", "qty", "amount"],
                          "properties": {"po": po, "line": line, "qty": qty, "amount": amount}}}
    ])
}

/// Result returned to the model, written so it can be relayed to the user.
#[derive(Debug, PartialEq, Eq)]
pub enum ToolResult {
    Ok(String),
    Rejected(String),
    BadCall(String),
}

/// Route one tool call through the kernel.
pub fn dispatch<S: Storage>(kernel: &mut Kernel<S>, name: &str, input: Value) -> ToolResult {
    let mut call = match input {
        Value::Object(map) => map,
        _ => return ToolResult::BadCall("tool input must be an object".into()),
    };
    call.insert("op".into(), Value::String(name.into()));
    let op: Operation = match serde_json::from_value(Value::Object(call)) {
        Ok(op) => op,
        Err(e) => return ToolResult::BadCall(format!("could not read `{name}` call: {e}")),
    };
    match kernel.submit(op) {
        Ok(event) => ToolResult::Ok(match event.assigned {
            Some(id) => format!("Created {id} (draft)."),
            None => "Posted.".into(),
        }),
        Err(r) => ToolResult::Rejected(r.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bizkernel_store::MemoryStore;

    #[test]
    fn every_tool_maps_to_an_operation() {
        let names: Vec<_> = tools().as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_owned()).collect();
        assert_eq!(names, ["create_po", "approve_po", "cancel_po", "post_goods_receipt", "post_invoice"]);
    }

    #[test]
    fn agent_calls_go_through_the_guardrail() {
        let mut k = Kernel::open(MemoryStore::new()).unwrap();
        let r = dispatch(&mut k, "create_po", json!({"vendor": 7, "lines": [{"material": 1, "qty": 10, "unit_price": 500}]}));
        assert_eq!(r, ToolResult::Ok("Created PO-4500000001 (draft).".into()));
        assert_eq!(dispatch(&mut k, "approve_po", json!({"po": 4500000001u64})), ToolResult::Ok("Posted.".into()));

        let r = dispatch(&mut k, "post_goods_receipt", json!({"po": 4500000001u64, "line": 1, "qty": 12}));
        assert_eq!(
            r,
            ToolResult::Rejected("PO-4500000001 line 1: receiving 12 would exceed the ordered quantity (10 still open)".into())
        );
    }

    #[test]
    fn malformed_calls_never_reach_the_kernel() {
        let mut k = Kernel::open(MemoryStore::new()).unwrap();
        assert!(matches!(dispatch(&mut k, "drop_tables", json!({})), ToolResult::BadCall(_)));
        assert!(matches!(dispatch(&mut k, "post_goods_receipt", json!({"po": 1, "line": 1, "qty": -5})), ToolResult::BadCall(_)));
        assert_eq!(k.store().records().len(), 0);
    }
}
