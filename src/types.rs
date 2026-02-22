use rust_decimal::Decimal;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Record {
    #[serde(rename = "type")]
    pub r#type: String,
    pub client: u16,
    pub tx: u32,
    pub amount: Decimal,
}

#[derive(Default)]
pub struct Client {
    pub available: Decimal,
    pub held: Decimal,
    pub locked: bool,
}

pub struct Transaction {
    pub client: u16,
    pub amount: Decimal,
    pub disputed: bool,
}
