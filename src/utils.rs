use crate::types::TimestampMs;
use near_sdk::{env};

pub fn current_time_ms() -> TimestampMs {
    env::block_timestamp() / 1_000_000
}

pub fn assert_condition<S: AsRef<str>>(condition: bool, message: S) {
    if condition {
        return;
    }

    env::panic_str(message.as_ref());
}
