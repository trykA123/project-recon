//! Enforces serialized response byte budgets and labels approximate token accounting.

use crate::{
    error::{self, Error, Result},
    validate,
};
use serde_json::{Value, json};

/// Labels estimated tokens and rejects a full JSON envelope beyond four bytes per budget unit.
pub fn bounded(mut data: Value, budget: usize) -> Result<Value> {
    let cap = validate::budget(budget)?;
    data["budget"] = json!(budget);
    data["tokenEstimateMethod"] = json!("utf8BytesDiv4");
    data["estimatedTokens"] = json!(0);
    for _ in 0..4 {
        let bytes = serde_json::to_vec(&error::envelope(data.clone()))?.len();
        data["estimatedTokens"] = json!(bytes.div_ceil(4));
    }
    if serde_json::to_vec(&error::envelope(data.clone()))?.len() > cap {
        return Err(Error::Budget);
    }
    Ok(data)
}
