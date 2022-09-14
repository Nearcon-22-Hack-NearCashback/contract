use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    serde::{Deserialize, Serialize},
    PublicKey,
};

pub type TimestampMs = u64;
pub type CashbackId = u64;

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Cashback {
    pub amount: u128,
    pub pub_key: PublicKey,
    pub creation_time: TimestampMs,
    pub is_sent: bool,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct AccountActivity {
    pub unpaid_cashback: u128,
    pub purchases_number: u64,
    pub roketo_stream_id: Option<String>,
}

impl Default for AccountActivity {
    fn default() -> Self {
        Self {
            unpaid_cashback: 0,
            purchases_number: 0,
            roketo_stream_id: None,
        }
    }
}
