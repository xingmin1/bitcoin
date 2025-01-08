#![allow(unused)]

use anyhow::Result;
use clap::{self, Subcommand};
use clap::{command, Parser};
use crate::transaction::Transaction;
use crate::utxo_set::UtxoSet;
use crate::wallet;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new blockchain
    Init {
        /// The address to send the genesis block's reward to
        #[clap(short, long)]
        address: String,
    },

    /// Send a transaction
    Send {
        /// The sender's address
        #[clap(short, long)]
        from: String,

        /// The recipient's address
        #[clap(short, long)]
        to: String,

        /// The amount to send
        #[clap(short, long)]
        amount: u32,
    },

    GetBalance {
        /// The address to check
        #[clap(short, long)]
        address: String,
    },

    /// Create a new wallet
    CreateWallet,

    /// List all addresses
    ListAddresses,

    /// List all blocks in the chain
    PrintChain,

    /// Verify the blockchain's integrity
    Verify,
}

#[derive(Debug)]
struct BlockchainApp<'a> {
    pub utxo_set: &'a mut UtxoSet,
}

impl<'a> BlockchainApp<'a> {
    fn new(utxo_set: &'a mut UtxoSet) -> Self {
        Self { utxo_set }
    }

    fn send(&mut self, from: String, to: String, amount: u32, path_prefix: &str) -> Result<()> {
        if !(wallet::validate_address(&from) && wallet::validate_address(&to)) {
            return Err(anyhow::anyhow!("Invalid address"));
        }

        let tx = Transaction::new_utxo_transaction(
            from.clone(),
            to,
            amount,
            self.utxo_set,
            path_prefix,
        );
        let coinbase_tx = Transaction::new_coinbase_tx(from, "".to_string());
        let block = self.utxo_set.blockchain.mine_block(vec![tx, coinbase_tx])?;
        self.utxo_set.update(&block, path_prefix);
        Ok(())
    }

    fn get_balance(&self, address: String, path_prefix: &str) -> Result<()> {
        let decoded = bs58::decode(&address).into_vec().unwrap();
        let pub_key_hash = &decoded[1..decoded.len() - 4];
        let balance = self
            .utxo_set
            .find_utxo(pub_key_hash, path_prefix)
            .into_iter()
            .map(|output| output.value)
            .sum::<u32>();
        println!("Balance of '{}': {}", address, balance);
        Ok(())
    }

    fn list_blocks(&self) {
        println!("{}", self.utxo_set.blockchain);
    }

    fn verify_chain(&self) -> Result<()> {
        self.utxo_set.blockchain.verify_chain()?;
        println!("Blockchain verification passed!");
        Ok(())
    }

    fn create_wallet(path_prefix: &str) -> String {
        let mut wallets = wallet::Wallets::new(path_prefix);
        let address = wallets.create_wallet();
        wallets.save_to_file(path_prefix);
        address
    }

    fn list_addresses(path_prefix: &str) {
        let wallets = wallet::Wallets::new(path_prefix);
        let addresses = wallets.get_addresses();
        for address in addresses {
            println!("{}", address);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::blockchain::Blockchain;

    use super::*;

    #[test]
    fn test() {
        let env = env_logger::Env::default()
            .filter_or("RUST_LOG", "debug")
            .write_style_or("RUST_LOG_STYLE", "always");

        env_logger::init_from_env(env);

        // 创建测试目录
        let _ = std::fs::remove_dir_all("data/test");
        std::fs::create_dir_all("data/test").unwrap();

        let path_prefix = "data/test";
        let a = BlockchainApp::create_wallet(path_prefix);
        let b = BlockchainApp::create_wallet(path_prefix);
        let c = BlockchainApp::create_wallet(path_prefix);

        // 创建区块链和 UTXO 集合
        let (blockchain, genesis) = Blockchain::new(a.clone(), path_prefix);
        let mut utxo_set = UtxoSet::new(blockchain);

        // 手动更新 UTXO 集合，确保包含创世区块的交易
        utxo_set.reindex(path_prefix);

        let mut app = BlockchainApp::new(&mut utxo_set);

        // 打印初始余额
        app.get_balance(a.clone(), path_prefix).unwrap();
        app.get_balance(b.clone(), path_prefix).unwrap();
        app.get_balance(c.clone(), path_prefix).unwrap();

        app.send(a.clone(), b.clone(), 15, path_prefix).unwrap();
        app.send(b.clone(), c.clone(), 5, path_prefix).unwrap();
        app.send(a.clone(), a.clone(), 10, path_prefix).unwrap();

        app.get_balance(a.clone(), path_prefix).unwrap();
        app.get_balance(b.clone(), path_prefix).unwrap();
        app.get_balance(c.clone(), path_prefix).unwrap();

        app.list_blocks();
        app.verify_chain().unwrap();

        BlockchainApp::list_addresses(path_prefix);

        std::fs::remove_dir_all("data/test").unwrap();
    }
}
