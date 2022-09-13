use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    json_types::U128,
    serde::{Deserialize, Serialize},
    PublicKey,
};

pub type TimestampMs = u64;
pub type CashbackId = u64;

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Cashback {
    pub amount: U128,
    pub pub_key: PublicKey,
    pub creation_time: TimestampMs,
}
