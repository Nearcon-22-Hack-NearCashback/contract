use ed25519_dalek::Verifier;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::LookupMap;
use near_sdk::json_types::U128;
use near_sdk::{env, ext_contract, near_bindgen, AccountId, PanicOnDefault, Promise, PublicKey};

mod types;
mod utils;

use types::{Cashback, CashbackId};
use utils::{assert_condition, current_time_ms};

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    next_cashback_id: CashbackId,
    active_cashbacks: LookupMap<CashbackId, Cashback>,
    claiming_key: String,
    linkdrop_contract: AccountId,
}

#[ext_contract(ext_linkdrop)]
pub trait ExtLinkDropContract {
    fn send(&mut self, public_key: PublicKey) -> Promise;
}

#[near_bindgen]
impl Contract {
    #[init]
    pub fn new(linkdrop: AccountId, key: String) -> Self {
        Self {
            next_cashback_id: 1,
            active_cashbacks: LookupMap::new(b"ac".to_vec()),
            claiming_key: key,
            linkdrop_contract: linkdrop,
        }
    }

    pub fn create(&mut self, pub_key: PublicKey, amount: U128) -> CashbackId {
        assert_condition(
            env::current_account_id() == env::signer_account_id(),
            "You're not allowed to create",
        );

        let cashback_id = self.next_cashback_id.clone();

        let cashback = Cashback {
            amount: amount,
            pub_key: pub_key,
            creation_time: current_time_ms(),
        };

        self.active_cashbacks.insert(&cashback_id, &cashback);

        self.next_cashback_id += 1;

        cashback_id
    }

    pub fn claim(&mut self, id: CashbackId, signature: Vec<u8>) {
        let cashback = self
            .active_cashbacks
            .remove(&id)
            .expect("No active cashback");

        let signature = ed25519_dalek::Signature::try_from(signature.as_ref())
            .expect("Signature should be a valid array of 64 bytes [13, 254, 123, ...]");

        // first byte contains CurveType, so we're removing it
        let public_key =
            ed25519_dalek::PublicKey::from_bytes(&cashback.pub_key.as_bytes()[1..]).unwrap();

        let verification_result = public_key.verify(&id.to_be_bytes(), &signature);

        assert_condition(verification_result.is_ok(), "Invalid signature");

        ext_linkdrop::ext(self.linkdrop_contract.clone())
            .with_attached_deposit(cashback.amount.0)
            .send(cashback.pub_key.clone());
    }

    pub fn update_claiming_key(&mut self, key: String) {
        assert_condition(
            env::current_account_id() == env::signer_account_id(),
            "You're not allowed to update this",
        );

        self.claiming_key = key;
    }

    pub fn get_claiming_key(&self) -> String {
        self.claiming_key.clone()
    }

    pub fn get_cashback_amount(&self, id: CashbackId) -> Option<U128> {
        let cashback = self.active_cashbacks.get(&id);

        match cashback {
            Option::None => None,
            Option::Some(c) => Some(c.amount),
        }
    }
}
