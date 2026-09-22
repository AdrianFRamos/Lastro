use std::{fs, path::PathBuf};

use lastro_protocol::StationEvent;
use serde_json::Value;

pub fn fixture_json() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../test-vectors/vectors.json");
    serde_json::from_slice(&fs::read(path).expect("read vectors.json")).expect("parse vectors.json")
}

pub fn event_bytes(name: &str) -> [u8; 276] {
    let fixture = fixture_json();
    let hex = fixture["events"][name]["event_bytes_hex"]
        .as_str()
        .expect("event bytes hex");
    hex::decode(hex)
        .expect("decode event hex")
        .try_into()
        .expect("276-byte event")
}

pub fn event(name: &str) -> StationEvent {
    StationEvent::decode(&event_bytes(name)).expect("valid event fixture")
}

pub fn hex_array<const N: usize>(value: &str) -> [u8; N] {
    hex::decode(value)
        .expect("decode fixture hex")
        .try_into()
        .expect("fixture has expected length")
}

pub fn fixture_hex_array<const N: usize>(path: &[&str]) -> [u8; N] {
    let fixture = fixture_json();
    let mut value = &fixture;
    for key in path {
        value = &value[*key];
    }
    hex_array(value.as_str().expect("fixture string"))
}

pub fn p256_order_minus(s: &[u8; 32]) -> [u8; 32] {
    let order = hex_array::<32>("ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551");
    let mut out = [0u8; 32];
    let mut borrow = 0u16;
    for index in (0..32).rev() {
        let minuend = order[index] as u16;
        let subtrahend = s[index] as u16 + borrow;
        if minuend >= subtrahend {
            out[index] = (minuend - subtrahend) as u8;
            borrow = 0;
        } else {
            out[index] = (256 + minuend - subtrahend) as u8;
            borrow = 1;
        }
    }
    assert_eq!(borrow, 0);
    out
}
