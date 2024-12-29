use crate::block::Block;

pub struct Blockchain {
    blocks: Vec<Block>,
}

impl Blockchain {
    pub fn new() -> Blockchain {
        Blockchain {
            blocks: vec![Block::new_genesis_block()],
        }
    }

    pub fn add_block(&mut self, data: Vec<u8>) {
        let prev_hash = self.blocks.last().unwrap().hash.clone();
        self.blocks.push(Block::new(data, prev_hash));
    }

    pub fn print_blocks(&self) {
        for block in &self.blocks {
            println!("{}", block);
            println!("----------------");
        }
    }
}
