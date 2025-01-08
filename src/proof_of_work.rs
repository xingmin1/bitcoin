use log::info;
use num_bigint::BigUint;
use sha2::{Digest, Sha256};

use crate::block::{Block, Hash};

const TARGET_BITS: usize = 16;

pub struct ProofOfWork<'a> {
    block: &'a Block,
    target: BigUint,
}

impl<'a> ProofOfWork<'a> {
    pub fn new(block: &'a Block) -> Self {
        let target = num_bigint::BigUint::from(1u32) << (256 - TARGET_BITS);
        Self { block, target }
    }

    pub fn run(&self) -> (BigUint, Hash) {
        let mut nonce = BigUint::default();
        let mut hasher = Sha256::new();
        let mut hash;
        let mut hash_int;

        info!("开始挖矿...");
        info!("交易数据: {}", self.block.transactions().iter().map(|tx| tx.to_string()).collect::<Vec<String>>().join("\n"));
        loop {
            hasher.update(self.prepare_data(&nonce));
            hash = hasher.finalize_reset();
            hash_int = BigUint::from_bytes_le(&hash);
            if hash_int < self.target {
                let hash = Hash::from(&hash);
                info!("挖矿成功!");
                info!("区块哈希: {}", hash);
                info!("随机数: {}", nonce);
                return (nonce, hash);
            }
            nonce += 1u32;
        }
    }

    fn prepare_data(&self, nonce: &BigUint) -> Vec<u8> {
        [
            self.block.prev_hash().as_bytes(),
            self.block.hash_transactions().as_bytes(),
            self.block.timestamp().to_le_bytes().as_ref(),
            TARGET_BITS.to_le_bytes().as_ref(),
            nonce.to_bytes_le().as_ref(),
        ]
        .concat()
    }

    pub fn validate(&self) -> bool {
        let data = self.prepare_data(self.block.nonce());
        let hash = Sha256::digest(&data);
        let hash_int = BigUint::from_bytes_le(&hash);
        hash_int < self.target
    }
}
