use ed25519_dalek::Verifier;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::LookupMap;
use near_sdk::is_promise_success;
use near_sdk::json_types::U128;
use near_sdk::serde_json::json;
use near_sdk::{
    env, ext_contract, near_bindgen, AccountId, PanicOnDefault, Promise, PromiseResult, PublicKey,
};

mod types;
mod utils;

use types::{AccountActivity, Cashback, CashbackId};
use utils::{assert_condition, current_time_ms};

/// 0.2N
const MIN_CASHBACK_ROKETO: u128 = 200_000_000_000_000_000_000_000;
const MIN_PURCHACES_NUMBER_ROKETO: u64 = 3;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    next_cashback_id: CashbackId,
    active_cashbacks: LookupMap<CashbackId, Cashback>,
    accounts: LookupMap<AccountId, AccountActivity>,
    claiming_key: String,
    linkdrop_contract: AccountId,
    roketo_contract: AccountId,
    ft_near_contract: AccountId,
}

#[ext_contract(ext_linkdrop)]
pub trait ExtLinkDropContract {
    fn send(&mut self, public_key: PublicKey) -> Promise;
}

#[ext_contract(ext_ft)]
pub trait ExtFungibleTokenContract {
    fn ft_transfer_call(&self, receiver_id: AccountId, amount: U128, msg: String);
}

#[near_bindgen]
impl Contract {
    #[init]
    pub fn new(linkdrop: AccountId, roketo: AccountId, ft_near: AccountId, key: String) -> Self {
        Self {
            next_cashback_id: 1,
            active_cashbacks: LookupMap::new(b"ac".to_vec()),
            accounts: LookupMap::new(b"a".to_vec()),
            claiming_key: key,
            linkdrop_contract: linkdrop, // testnet
            roketo_contract: roketo,     // streaming-r-v2.dcversus.testnet
            ft_near_contract: ft_near,   // wrap.testnet
        }
    }

    pub fn create(&mut self, pub_key: PublicKey, amount: U128) -> CashbackId {
        assert_condition(
            env::current_account_id() == env::signer_account_id(),
            "You're not allowed to create",
        );

        let cashback_id = self.next_cashback_id.clone();

        let actual_amount = amount.0 / 2;

        let cashback = Cashback {
            amount: actual_amount,
            pub_key,
            creation_time: current_time_ms(),
            is_sent: false,
        };

        self.active_cashbacks.insert(&cashback_id, &cashback);

        self.next_cashback_id += 1;

        cashback_id
    }

    pub fn claim(&mut self, id: CashbackId, signature: Vec<u8>) {
        let mut cashback = self.active_cashbacks.get(&id).expect("No active cashback");

        assert_condition(!cashback.is_sent, "Cashback linkdrop is already sent");

        cashback.is_sent = true;
        self.active_cashbacks.insert(&id, &cashback);

        let signature = ed25519_dalek::Signature::try_from(signature.as_ref())
            .expect("Signature should be a valid array of 64 bytes [13, 254, 123, ...]");

        // first byte contains CurveType, so we're removing it
        let public_key =
            ed25519_dalek::PublicKey::from_bytes(&cashback.pub_key.as_bytes()[1..]).unwrap();

        let verification_result = public_key.verify(&id.to_be_bytes(), &signature);

        assert_condition(verification_result.is_ok(), "Invalid signature");

        ext_linkdrop::ext(self.linkdrop_contract.clone())
            .with_attached_deposit(cashback.amount)
            .send(cashback.pub_key.clone())
            .then(Self::ext(env::current_account_id()).on_claim(id.clone()));
    }

    #[private]
    pub fn on_claim(&mut self, id: CashbackId) {
        let is_success = is_promise_success();

        match is_success {
            // if claim wasn't successful - return back
            false => {
                let mut cashback = self.active_cashbacks.get(&id).expect("No active cashback");

                cashback.is_sent = false;

                self.active_cashbacks.insert(&id, &cashback);
            }
            _ => {}
        };
    }

    pub fn log_claim(
        &mut self,
        account_id: AccountId,
        claimed_amount: U128,
        cashback_id: CashbackId,
    ) {
        assert_condition(
            env::current_account_id() == env::signer_account_id(),
            "You're not allowed to log",
        );

        let cashback = self
            .active_cashbacks
            .remove(&cashback_id)
            .expect("No active cashback");

        assert_condition(
            cashback.amount > claimed_amount.0,
            "Claimed amount can't be bigger than cashback",
        );

        let unpaid_amount = cashback.amount - claimed_amount.0;

        let mut activity = self.accounts.get(&account_id).unwrap_or_default();

        activity.purchases_number += 1;
        activity.unpaid_cashback += unpaid_amount;

        let is_roketo_streaming = self.check_roketo_streaming(&activity);

        if (is_roketo_streaming) {
            activity.unpaid_cashback -= MIN_CASHBACK_ROKETO;
            self.process_roketo_activity(account_id.clone(), &activity);
        }

        self.accounts.insert(&account_id, &activity);
    }

    fn check_roketo_streaming(&self, activity: &AccountActivity) -> bool {
        activity.purchases_number >= MIN_PURCHACES_NUMBER_ROKETO
            && activity.unpaid_cashback >= MIN_CASHBACK_ROKETO
    }

    /// stream wNear tokens through Roketo
    fn process_roketo_activity(&self, account_id: AccountId, activity: &AccountActivity) {
        let roketo_promise = match &activity.roketo_stream_id {
            Option::Some(id) => ext_ft::ext(self.ft_near_contract.clone())
                .with_attached_deposit(1)
                .ft_transfer_call(
                    account_id.clone(),
                    U128::from(MIN_CASHBACK_ROKETO),
                    json!({
                        "Deposit": {
                            "stream_id": id.clone()
                        }
                    })
                    .to_string(),
                ),
            Option::None => ext_ft::ext(self.ft_near_contract.clone())
                .with_attached_deposit(1)
                .ft_transfer_call(
                    account_id.clone(),
                    U128::from(MIN_CASHBACK_ROKETO),
                    json!({
                        "Create": {
                            "request": {
                                "owner_id": env::current_account_id(),
                                "receiver_id": account_id.clone(),
                                "tokens_per_sec": 3
                            }
                        }
                    })
                    .to_string(),
                ),
        };

        roketo_promise.then(
            Self::ext(env::current_account_id())
                .on_log_claim(account_id.clone(), U128::from(MIN_CASHBACK_ROKETO)),
        );
    }

    #[private]
    pub fn on_log_claim(&mut self, account_id: AccountId, roketo_cashback: U128) {
        let is_success = is_promise_success();

        match is_success {
            // if log wasn't successful - return back roketo cashback
            false => {
                let mut activity = self.accounts.get(&account_id).expect("No account activity");

                activity.unpaid_cashback += roketo_cashback.0;

                self.accounts.insert(&account_id, &activity);
            }
            _ => {}
        };
    }

    pub fn update_claiming_key(&mut self, key: String) {
        assert_condition(
            env::current_account_id() == env::signer_account_id(),
            "You're not allowed to update this",
        );

        self.claiming_key = key;
    }

    pub fn update_linkdrop(&mut self, contract_id: AccountId) {
        assert_condition(
            env::current_account_id() == env::signer_account_id(),
            "You're not allowed to update this",
        );

        assert_condition(
            env::is_valid_account_id(contract_id.as_bytes()),
            "Isn't valid contract",
        );

        self.linkdrop_contract = contract_id;
    }
    pub fn update_roketo(&mut self, contract_id: AccountId) {
        assert_condition(
            env::current_account_id() == env::signer_account_id(),
            "You're not allowed to update this",
        );

        assert_condition(
            env::is_valid_account_id(contract_id.as_bytes()),
            "Isn't valid contract",
        );

        self.roketo_contract = contract_id;
    }

    pub fn get_claiming_key(&self) -> String {
        self.claiming_key.clone()
    }

    pub fn get_cashback_amount(&self, id: CashbackId) -> Option<U128> {
        let cashback = self.active_cashbacks.get(&id);

        match cashback {
            Option::None => None,
            Option::Some(c) => Some(U128::from(c.amount)),
        }
    }
}
