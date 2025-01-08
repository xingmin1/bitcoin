use secp256k1::{rand::rngs::OsRng, Secp256k1};
use serde::{Deserialize, Serialize};
use sha2::Digest;

const VERSION: u8 = 0x00;

#[derive(Debug, Clone, Serialize, PartialEq, Deserialize)]
pub struct Wallet {
    pub private_key: secp256k1::SecretKey,
    pub public_key: Vec<u8>,
}

impl Wallet {
    /// 创建一个新的钱包(公私钥对)
    pub fn new() -> Self {
        let (private_key, public_key) = new_key_pair();
        Self {
            private_key,
            public_key,
        }
    }

    /// 根据钱包公钥生成钱包地址
    pub fn address(&self) -> String {
        let pub_key_hash = hash_pub_key(&self.public_key);
        let versioned_payload = [VERSION].to_vec();
        let payload = [versioned_payload, pub_key_hash].concat();
        let checksum = checksum(&payload);
        let unencoded_address = [payload, checksum].concat();
        bs58::encode(unencoded_address).into_string()
    }
}

/// 检验校验和是否正确
pub fn validate_address(address: &str) -> bool {
    let decoded = bs58::decode(address).into_vec().unwrap();
    let payload = &decoded[..decoded.len() - 4];
    let checksum_bytes = &decoded[decoded.len() - 4..];
    let calculated_checksum = checksum(payload);
    checksum_bytes == calculated_checksum
}

pub fn hash_pub_key(pub_key: &[u8]) -> Vec<u8> {
    ripemd::Ripemd160::digest(sha2::Sha256::digest(pub_key)).to_vec()
}

fn checksum(payload: &[u8]) -> Vec<u8> {
    let first_hash = sha2::Sha256::digest(payload);
    let second_hash = sha2::Sha256::digest(first_hash);
    second_hash[..4].to_vec()
}

fn new_key_pair() -> (secp256k1::SecretKey, Vec<u8>) {
    let secp = Secp256k1::new();
    let (private_key, public_key) = secp.generate_keypair(&mut OsRng);
    (private_key, public_key.serialize().to_vec())
}
