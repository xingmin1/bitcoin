use anyhow::Result;
use clap::{self, Subcommand};
use clap::{command, Parser};
mod block;
mod blockchain;

use blockchain::Blockchain;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new blockchain
    Init,

    /// Add a new block with the given data
    Add {
        /// The data to store in the block
        #[arg(short, long)]
        data: String,
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
    fn new() -> Result<Self> {
        Ok(Self {
            chain: Blockchain::new(),
        })
    }

    fn add_block(&mut self, data: Vec<u8>) -> Result<()> {
        self.chain.add_block(data)?;
        println!("Block added successfully!");
        Ok(())
    }

    fn list_blocks(&self) -> Result<()> {
        println!("{}", self.chain);
        println!("Total blocks: {}", self.chain.height());
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
    let mut app = BlockchainApp::new()?;

    match &cli.command {
        Commands::Init => {
            println!("Initialized new blockchain");
            app.list_blocks()?;
        }

        Commands::Add { data } => {
            app.add_block(data.as_bytes().to_vec())?;
        }

        Commands::List => {
            app.list_blocks()?;
        }

        Commands::Verify => {
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
        let mut app = BlockchainApp::new()?;

        // Add some blocks
        app.add_block(b"Test Block 1".to_vec())?;
        app.add_block(b"Test Block 2".to_vec())?;

        // Verify chain
        app.verify_chain()?;

        Ok(())
    }
}
