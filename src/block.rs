use sha2::{self, Digest};
use std::time::SystemTime;

pub struct Block {
    timestamp: i64,
    data: Vec<u8>,
    prev_hash: Hash,
    pub hash: Hash,
}

impl Block {
    pub fn new(data: Vec<u8>, prev_hash: Hash) -> Block {
        let mut block = Block {
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            data,
            prev_hash,
            hash: Hash::default(),
        };
        block.set_hash();
        block
    }

    fn set_hash(&mut self) {
        let data = [
            self.timestamp.to_be_bytes().to_vec(),
            self.data.clone(),
            self.prev_hash.0.clone(),
        ]
        .concat();
        self.hash = Hash::new(data);
    }

    pub fn new_genesis_block() -> Block {
        Block::new(b"Genesis Block".to_vec(), Hash::default())
    }
}

impl std::fmt::Display for Block {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(f, "timestamp: {},", self.timestamp)?;
        writeln!(f, "data: {},", String::from_utf8_lossy(&self.data))?;
        writeln!(f, "prev_hash: {},", self.prev_hash)?;
        writeln!(f, "hash: {},", self.hash)?;
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct Hash(Vec<u8>);

impl Hash {
    fn new(data: Vec<u8>) -> Hash {
        let mut hasher = sha2::Sha256::new();
        hasher.update(data);
        Hash(hasher.finalize().to_vec())
    }
}

impl std::fmt::Display for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for byte in &self.0 {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

impl std::ops::Deref for Hash {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
