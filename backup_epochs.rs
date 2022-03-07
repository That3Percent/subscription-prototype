use std::{
    cmp::{Ord, Ordering},
    collections::HashMap,
};

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
type Grt = u128;
type Epoch = u128;
type Tick = u128;
type Block = u128;
type Account = u128;
type Count = u128;

// For simplicity, but this could be a simple tick
#[derive(Default, PartialEq, Eq, Debug, Hash)]
pub struct Timestamp {
    epoch: Epoch,
    tick: Tick,
}

impl Timestamp {
    pub const TICKS_PER_EPOCH: Tick = 1000;
    fn new(epoch: Epoch, tick: Tick) -> Self {
        assert!(epoch != 0);
        assert!(tick < Self::TICKS_PER_EPOCH);
        Self { epoch, tick }
    }
    fn from_block(block: Block) -> Self {
        let epoch = (block / Self::TICKS_PER_EPOCH) + 1;
        let tick = block % Self::TICKS_PER_EPOCH;
        Self::new(epoch, tick)
    }

    fn to_block(&self) -> Block {
        (self.epoch - 1) * Self::TICKS_PER_EPOCH + self.tick
    }
}

impl PartialOrd for Timestamp {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(match self.epoch.cmp(&other.epoch) {
            Ordering::Greater => Ordering::Greater,
            Ordering::Less => Ordering::Less,
            Ordering::Equal => self.tick.cmp(&other.tick),
        })
    }
}

impl Ord for Timestamp {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.epoch.cmp(&other.epoch) {
            Ordering::Greater => Ordering::Greater,
            Ordering::Less => Ordering::Less,
            Ordering::Equal => self.tick.cmp(&other.tick),
        }
    }
}

// TODO: How would you change the price?
// You might include the slush & balance struct per fee schedule or something?
// But there are still corner cases, like withdrawal where you would need to know
// at what fee level(s) they subscribed.
//
// Another thought here is that there is no verifability that the SLA
// is being upheld. At least customers can switch.
pub struct Subscription {
    service: Account,
    balance: Grt,
    last_collected_epoch: Epoch,
    current_block: Block,
    slush: Map<Epoch, Grt>,
    paid_until: Map<Account, Block>,
    accounts_expiring: Map<Epoch, Count>,
    currently_active_accounts: Count,
}

impl Subscription {
    // This API would not exist on a real chain.
    pub fn set_current_block(&mut self, block: Block) {
        assert!(block > self.current_block);
        self.current_block = block;
    }

    pub fn top_off(&mut self, account: Account, amount: Grt) {
        self.balance += amount;
        // Find out how much they need to round out to an epoch,
        // and add it to the slush for their last epoch (which may
        // be this one if unspecified)
        // Add ticks to their time

        // Add epochs to their time

        // Add the remaining slush to the final epoch.
        todo!()
    }
    pub fn current_time(&self) -> Timestamp {
        Timestamp::from_block(self.current_block())
    }
    pub fn current_block(&self) -> Block {
        self.current_block
    }
    pub fn is_active(&self, account: Account) -> bool {
        let paid_until = self.paid_until.get(&account).copied().unwrap_or_default();
        paid_until > self.current_block()
    }
    // Moves funds into the service account for a given epoch,
    // if it has not yet been collected.
    // TODO: Actually, we do have to step through these linearly.
    // That way we can ensure that expired accounts are removed.
    fn collect_epoch(&mut self) {
        // TODO: We may need linked lists to step through and keep constant-time
        // updates. A skiplist, actually.
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{thread_rng, Rng, RngCore};
    #[test]
    fn block_to_epoch() {
        for _ in 0..1000 {
            let rand_epoch = thread_rng().gen_range(1..1000);
            let rand_tick = thread_rng().gen_range(0..Timestamp::TICKS_PER_EPOCH);
            let timestamp = Timestamp::new(rand_epoch, rand_tick);
            let block = timestamp.to_block();
            assert_eq!(Timestamp::from_block(block), timestamp);
        }
    }
}
