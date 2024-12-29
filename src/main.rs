use blockchain::Blockchain;

mod block;
mod blockchain;

fn main() {
    let mut blockchain = Blockchain::new();
    blockchain.add_block(b"Send 1 BTC to Ivan".to_vec());
    blockchain.add_block(b"Send 2 more BTC to Ivan".to_vec());
    blockchain.print_blocks();
}
