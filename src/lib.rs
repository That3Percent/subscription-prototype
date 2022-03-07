mod skiplist;
use skiplist::SkipList;
use std::{cmp::Ord, collections::HashMap};

// New design:
// Skiplist is appendable, immutable and has up to 1 entry per block
// The data in the skiplist is the increase/decrease in GRT price per block
// When adding collateral, the user finds the current block in the list (or their last block), and adds to increase
// as well as the expiry, and adds to decrease.
// On unsubscribe, the user finds their last subscription (there may be more than one due to price changes)
// and undoes the above, accounting for the fact that the current block will have moved forward.
// When consuming time, the skiplist blocks are removed.

type Map<K, V> = HashMap<K, V>;

// These can be smaller, but using same type for convenience
type Block = i128;
type Account = [u8; 20];

// TODO: Consts
// By doing 1 epoch per hour there would be ~> 265 blocks per epoch.
// Then if the skiplist has 16 levels, the fastest level would skip
// 7 years at a time, assuming full utilization.
// This is clearly overkill.
// If we have epochs the simplest thing to do is to have the funds from the
// first epoch be irrecoverable. Otherwise we create the slush fund, which is ok I guess.
// This seems fine to go down to 1 epoch per 10 minutes or so?

#[derive(Debug)]
struct Subscription {
    start_block: Block,
    end_block: Block,
    price_per_block: i128,
}

struct Collector {
    last_collected_block: Block,
    current_fee: i128,
    balance: i128,
    // Keeps track of token transfer. This is not necessary for a real blockchain.
    service_balance: i128,
}

impl Collector {
    pub fn new() -> Self {
        Self {
            last_collected_block: 0,
            current_fee: 0,
            balance: 0,
            service_balance: 0,
        }
    }

    fn collect_one(&mut self, block: i128) {
        let num_blocks = block - self.last_collected_block;
        let fee = self.current_fee * num_blocks;
        self.balance -= fee;
        self.service_balance += fee;
        self.last_collected_block = block;
    }
}

pub struct SubscriptionManager {
    collector: Collector,
    price_per_block: i128,
    current_block: Block,
    changes: SkipList,
    subscriptions: Map<Account, Vec<Subscription>>,
    // TODO: Consider having a minimum time to subscribe to prevent
    // subscribing for 1 block so that the next part of the transaction
    // shows as active. Consider also having the first block be the next
    // for the same reason to prevent MEV weirdness.
    // TODO: Fix rounding here? May not be worth the gas.
    //tax: Grt,
}

impl SubscriptionManager {
    pub fn new() -> Self {
        Self {
            collector: Collector::new(),
            price_per_block: 0,
            current_block: 0,
            changes: SkipList::new(),
            subscriptions: Map::new(),
        }
    }

    // This API would not exist on a real chain.
    pub fn set_current_block(&mut self, block: Block) {
        assert!(block > self.current_block);
        self.current_block = block;
    }

    pub fn current_block(&self) -> Block {
        self.current_block
    }

    // Callable only by the service
    pub fn set_price_per_block(&mut self, price: i128) {
        self.price_per_block = price;
    }

    pub fn top_off(&mut self, account: Account, amount: i128) {
        let price_per_block = self.price_per_block;
        let current_block = self.current_block();
        let num_blocks = amount / price_per_block;
        if num_blocks == 0 {
            // TODO: Actually we want to allow the consumer specify the price
            // so they are resiliant to recent changes. Or, we could schedule
            // changes in price for the future.
            return;
        }

        self.collector.balance += amount;

        let subs = self.subscriptions.entry(account).or_default();

        let start_block = subs
            .last()
            .map(|s| s.end_block)
            .unwrap_or_default()
            .max(current_block + 1);
        let end_block = start_block + num_blocks;

        subs.push(Subscription {
            start_block,
            end_block,
            price_per_block,
        });

        *self.changes.get_or_insert_mut(start_block) += price_per_block as i128;
        *self.changes.get_or_insert_mut(end_block) -= price_per_block as i128;
    }

    pub fn is_active(&self, account: Account) -> bool {
        let subs = if let Some(subs) = self.subscriptions.get(&account) {
            subs
        } else {
            return false;
        };

        for sub in subs.iter().rev() {
            if sub.start_block <= self.current_block {
                if sub.end_block > self.current_block {
                    return true;
                }
                return false;
            }
        }
        return false;
    }

    pub fn collect(&mut self) {
        // Process all changes
        let mut changes_processed = 0;
        // TODO: For security we need to cap the changes
        // processed to some const
        for (&block, &delta) in self.changes.iter() {
            if block > self.current_block() {
                break;
            }
            self.collector.collect_one(block);
            self.collector.current_fee += delta;
            changes_processed += 1;
        }

        self.changes.truncate_front(changes_processed);
        self.collector.collect_one(self.current_block());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(id: u8) -> Account {
        [id; 20]
    }

    #[test]
    pub fn one_complete_subscription() {
        let mut subs = SubscriptionManager::new();
        subs.price_per_block = 10;

        // Buy blocks 6-16
        subs.set_current_block(5);
        subs.top_off(account(1), 100);

        subs.set_current_block(100);
        subs.collect();

        assert_eq!(subs.collector.service_balance, 100);
    }

    #[test]
    pub fn subscription_is_active_for_required_blocks() {
        let mut subs = SubscriptionManager::new();
        subs.price_per_block = 10;

        let account = account(1);

        // Buy blocks 6-16
        subs.set_current_block(5);
        subs.top_off(account, 100);

        assert_eq!(false, subs.is_active(account));
        for i in 6..16 {
            subs.set_current_block(i);
            assert_eq!(true, subs.is_active(account));
        }
        subs.set_current_block(16);
        assert_eq!(false, subs.is_active(account));
    }

    #[test]
    pub fn loop_collect() {
        let mut subs = SubscriptionManager::new();
        subs.price_per_block = 5;

        // Buy blocks 6-16
        subs.set_current_block(1);
        subs.top_off(account(1), 100);

        for i in 2..30 {
            subs.set_current_block(i);
            subs.collect();
        }

        assert_eq!(subs.collector.service_balance, 100);
    }

    #[test]
    pub fn overlapping_subscriptions() {
        let mut subs = SubscriptionManager::new();
        subs.price_per_block = 10;

        // Buy blocks 6-15 (inclusive)
        subs.set_current_block(5);
        subs.top_off(account(1), 100);

        // Buy blocks 11-31 (inclusive)
        subs.set_current_block(10);
        subs.top_off(account(2), 200);

        // Collect:
        //   6-10  (5 blocks) @ 1 +
        //   11-15 (6 blocks) @ 2 +
        //   16-18 (3 blocks) @ 1
        // = 19 subscribed blocks = * 10 = 190
        subs.set_current_block(19);
        subs.collect();

        assert_eq!(subs.collector.service_balance, 180);
    }
}
