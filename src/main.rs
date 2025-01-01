use anyhow::Result;
use clap::{self, Subcommand};
use clap::{command, Parser};
mod block;
mod blockchain;
mod proof_of_work;
mod transaction;

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

    /// Add a new block with the given data
    // Add {
    //     /// The data to store in the block
    //     #[arg(short, long)]
    //     data: String,
    // },
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

    /// List all blocks in the chain
    List,

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
        let tx = Transaction::new_utxo_transaction(from, to, amount, &self.chain);
        self.chain.mine_block(vec![tx])?;
        Ok(())
    }

    fn get_balance(&self, address: String) -> Result<()> {
        let balance = self
            .chain
            .find_utxo(&address)
            .into_iter()
            .map(|out| out.value)
            .sum::<u32>();
        println!("Balance of '{}': {}", address, balance);
        Ok(())
    }

    fn list_blocks(&self) -> Result<()> {
        println!("{}", self.chain);
        Ok(())
    }

    fn verify_chain(&self) -> Result<()> {
        self.chain.verify_chain()?;
        println!("Blockchain verification passed!");
        Ok(())
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init {address} => {
            println!("Initialized new blockchain");
            
            let app = BlockchainApp::new(address)?;
            app.list_blocks()?;
        }

        Commands::Send { from, to, amount } => {
            let mut app = BlockchainApp::new(from.clone())?;
            app.send(from, to, amount)?;
            app.list_blocks()?;
        }

        Commands::GetBalance { address } => {
            let app = BlockchainApp::new(address.clone())?;
            app.get_balance(address)?;
        }

        Commands::List => {
            let app = BlockchainApp::new("".to_string())?;
            app.list_blocks()?;
        }

        Commands::Verify => {
            let app = BlockchainApp::new("".to_string())?;
            app.verify_chain()?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blockchain_app() -> Result<()> {
        let mut app = BlockchainApp::new("Alice".to_string())?;

        // Add some blocks
        app.send("Alice".to_string(), "Bob".to_string(), 10)?;
        app.send("Bob".to_string(), "Charlie".to_string(), 5)?;

        // Verify chain
        app.verify_chain()?;

        // List blocks
        app.list_blocks()?;

        Ok(())
    }
}
