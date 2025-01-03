use anyhow::Result;
use clap::{self, Subcommand};
use clap::{command, Parser};
mod block;
mod blockchain;
mod proof_of_work;
mod transaction;
mod wallet;

use blockchain::Blockchain;
use transaction::Transaction;

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
struct BlockchainApp {
    chain: Blockchain,
}

impl BlockchainApp {
    fn new(address: String) -> Result<Self> {
        Ok(Self {
            chain: Blockchain::new(address),
        })
    }

    fn send(&mut self, from: String, to: String, amount: u32) -> Result<()> {
        if !(wallet::validate_address(&from) && wallet::validate_address(&to)) {
            return Err(anyhow::anyhow!("Invalid address"));
        }

        let tx = Transaction::new_utxo_transaction(from, to, amount, &self.chain);
        self.chain.mine_block(vec![tx])?;
        Ok(())
    }

    fn get_balance(&self, address: String) -> Result<()> {
        let decoded = bs58::decode(&address).into_vec().unwrap();
        let pub_key_hash = &decoded[1..decoded.len() - 4];
        let balance = self
            .chain
            .find_utxo(pub_key_hash)
            .into_iter()
            .map(|out| out.value)
            .sum::<u32>();
        println!("Balance of '{}': {}", address, balance);
        Ok(())
    }

    fn list_blocks(&self) {
        println!("{}", self.chain);
    }

    fn verify_chain(&self) -> Result<()> {
        self.chain.verify_chain()?;
        println!("Blockchain verification passed!");
        Ok(())
    }

    fn create_wallet() -> String {
        let mut wallets = wallet::Wallets::new();
        let address = wallets.create_wallet();
        wallets.save_to_file();
        address
    }

    fn list_addresses() {
        let wallets = wallet::Wallets::new();
        let addresses = wallets.get_addresses();
        for address in addresses {
            println!("{}", address);
        }
    }
}

fn main() -> Result<()> {
    // The `Env` lets us tweak what the environment
    // variables to read are and what the default
    // value is if they're missing
    let env = env_logger::Env::default()
        .filter_or("LOG_LEVEL", "warn")
        .write_style_or("LOG_STYLE", "always");

    env_logger::init_from_env(env);

    let cli = Cli::parse();

    match cli.command {
        Commands::Init { address } => {
            println!("Initialized new blockchain");

            let app = BlockchainApp::new(address)?;
            app.list_blocks();
        }

        Commands::Send { from, to, amount } => {
            let mut app = BlockchainApp::new(from.clone())?;
            app.send(from, to, amount)?;
            app.list_blocks();
        }

        Commands::GetBalance { address } => {
            let app = BlockchainApp::new(address.clone())?;
            app.get_balance(address)?;
        }

        Commands::PrintChain => {
            let app = BlockchainApp::new("".to_string())?;
            app.list_blocks();
        }

        Commands::Verify => {
            let app = BlockchainApp::new("".to_string())?;
            app.verify_chain()?;
        }

        Commands::CreateWallet => {
            println!("New wallet created with address: {}", BlockchainApp::create_wallet());
        }

        Commands::ListAddresses => {
            BlockchainApp::list_addresses();
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use log::warn;

    use super::*;
    use std::fs;

    #[test]
    fn test() {
        let env = env_logger::Env::default()
            .filter_or("RUST_LOG", "debug")
            .write_style_or("RUST_LOG_STYLE", "always");

        env_logger::init_from_env(env);

        let _ = fs::remove_file("wallets.dat").inspect_err(|e| {
            warn!("Failed to remove wallets.dat: {}", e);
        });
        let _ = fs::remove_dir_all("blockchain.db").inspect_err(|e| {
            warn!("Failed to remove blockchain.db: {}", e);
        });
        let a = BlockchainApp::create_wallet();
        let b = BlockchainApp::create_wallet();
        let c = BlockchainApp::create_wallet();
        let mut app = BlockchainApp::new(a.clone()).unwrap();
        app.send(a.clone(), b.clone(), 15).unwrap();
        app.send(b.clone(), c.clone(), 5).unwrap();
        app.get_balance(a.clone()).unwrap();
        app.get_balance(b.clone()).unwrap();
        app.get_balance(c.clone()).unwrap();

        app.list_blocks();
        app.verify_chain().unwrap();

        BlockchainApp::list_addresses();

        let _ = fs::remove_dir("wallets.dat");
        let _ = fs::remove_dir("blockchain.db");
    }
}
