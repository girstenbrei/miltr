#![no_main]

use async_trait::async_trait;
use libfuzzer_sys::fuzz_target;

use bytes::BytesMut;
use miltr_common::{
        actions::{Action, Continue},
};
use miltr_server::{Milter, fuzzing::fuzz_parse};

struct DecodingMilter;

#[async_trait]
impl Milter for DecodingMilter {
    type Error = &'static str;
    async fn abort(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

fuzz_target!(|data: &[u8]| {
    let mut buffer = BytesMut::from(data);
    let _decoded = fuzz_parse(&mut buffer);
});
