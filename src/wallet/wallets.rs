use std::collections::HashMap;

use crate::wallet::hash_pub_key;

use super::wallet::Wallet;
use log::info;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const WALLETS_FILE: &str = "wallets.dat";

#[derive(Serialize, Deserialize, Default)]
pub struct Wallets {
    pub wallets: HashMap<String, Wallet>,
}

impl Wallets {
    /// 创建一个新的钱包集合
    pub fn new(path_prefix: &str) -> Self {
        Self::load_from_file(path_prefix).unwrap_or_default()
    }

    /// 添加一个新的钱包到集合中
    pub fn create_wallet(&mut self) -> String {
        let wallet = Wallet::new();
        let address = wallet.address();
        info!("创建新钱包");
        info!("地址: {}", address);
        info!("公钥: {:x?}", wallet.public_key);
        info!("私钥: {:x?}", wallet.private_key);
        info!("公钥哈希: {:x?}", hash_pub_key(&wallet.public_key));
        self.wallets.insert(address.clone(), wallet);
        address
    }

    /// 获取所有钱包地址
    pub fn get_addresses(&self) -> Vec<&String> {
        self.wallets.keys().collect()
    }

    /// 获取指定地址的钱包
    pub fn get_wallet(&self, address: &str) -> Option<&Wallet> {
        self.wallets.get(address)
    }

    /// 从文件中加载钱包集合
    fn load_from_file(path_prefix: &str) -> Result<Self, WalletsError> {
        let path = format!("{}/{}", path_prefix, WALLETS_FILE);
        let data = std::fs::read(path)?;
        let wallets = bincode::deserialize(&data)?;
        Ok(wallets)
    }

    pub fn save_to_file(&self, path_prefix: &str) {
        let path = format!("{}/{}", path_prefix, WALLETS_FILE);
        let data = bincode::serialize(&self.wallets).expect("Failed to serialize wallets");
        std::fs::write(path, data).expect("Failed to write wallets to file");
    }
}

#[derive(Debug, Error)]
pub enum WalletsError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Failed to serialize wallets")]
    SerializeWalletsError(#[from] bincode::Error),
}
