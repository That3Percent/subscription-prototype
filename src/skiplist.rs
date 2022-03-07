use crate::Block;
use std::mem::transmute;

// TODO: An efficient version probably has something like:
// Each level packs 4 64 bit indexes per word
// Except the last level which has a 128bit fee delta.
// This way, only 1 in 4 would require 2 or more words allocated.
// Read is cheap, and modify costs 1/4 write.
//
// Each leaf node needs to store:
//   u128 delta
//   u64 block
// So the below is incorrect.
// There is some worry about the 4 billion node limit to use u32 for indices.
// But, consider that 4 billion blocks = 2042 years.
// If there are epochs, it can be much longer. They can upgrade the contract by then.
// Note that "an attacker" can still exhaust the range before that time by repeatedly
// executing subscribe/unsubscribe (at their own multi-billion dollar expense!)
// but it is possible to have a freelist
//
// For a max level of 14 (3 words having 4 levels, and 1 word with 2 levels)
// there would be an average of 16k nodes skipped at the highest level.
// 80,000 (store 4 words)
// 70,000 (modify 14 words)
// Potentially * 2 (very unlikely)
// + read & execute costs
// + 20,000 to store subscription
// + 21,000 base transaction cost
// + ? calldata
// + ~100,000 Erc-20 transfer
// ~= 450,000 Worst case (probability 1 per 200M)
// This would cost ~$200
//
// 20,000 (store 4 words)
// 10,000 (modify 2 words)
// * 2
// + read & execute costs
// + 20,000 to store subscription
// + 21,000 base transaction cost
// + ? calldata
// + ~100,000 Erc-20 transfer
// ~= 201,000 Typical case
//
// Problem: Gas estimation being off can make it much more likely that only nodes with low skip exist.
// In this case, the counter should not advance (requiring the high-skip node to be created)
// One way to counter that may be to have the router update the skiplist, and consumers post
// their subscriptions to a queue. That may use more gas overall, though.

// This needs to be the efficient version, but leaving out the complex implementation
// on account of simplicity. The real implementation would be gas efficient on insert

struct Word([u8; 32]);

impl Word {
    fn as_u32s(self) -> [u32; 8] {
        unsafe { transmute(self.0) }
    }
}

#[derive(Clone, Debug)]
pub struct SkipList {
    keys: Vec<Block>,
    values: Vec<i128>,
}

struct Levels0_2 {
    level0: u32,
    level1: u32,
}

impl SkipList {
    pub fn new() -> Self {
        Self {
            keys: Vec::new(),
            values: Vec::new(),
        }
    }
    pub fn truncate_front(&mut self, index: usize) {
        self.keys.drain(..index);
        self.values.drain(..index);
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Block, &i128)> {
        self.keys.iter().zip(self.values.iter())
    }

    pub fn get_or_insert_mut(&mut self, k: Block) -> &mut i128 {
        let i = match self.keys.binary_search(&k) {
            Ok(i) => i,
            Err(i) => {
                self.values.insert(i, 0);
                self.keys.insert(i, k);
                i
            }
        };
        &mut self.values[i]
    }
}
