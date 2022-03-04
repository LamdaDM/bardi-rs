use std::{cell::{Cell, RefCell}, rc::Rc};

use rust_decimal::Decimal;

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use crate::{AssetPool, Asset};

    #[test]
    fn asset_pool_changes() {
        let asset_pool = AssetPool::new();

        let idx = asset_pool.load(Asset::new(Decimal::new(5090, 2)));
        
        if !(asset_pool.mutate(idx, Decimal::new(-90, 2))) {
            panic!("Failed to mutate asset {}!", idx);
        }

        let actual = asset_pool.get(idx);
        let expected = Some(Decimal::new(5000, 2));

        assert_eq!(actual, expected)
    }

    #[test]
    fn asset_pool_capture() {
        let asset_pool = AssetPool::new();

        let values = vec![ 
            Decimal::new(42, 3), 
            Decimal::new(900, 0), 
            Decimal::new(500, 1) 
        ];

        values.iter().for_each(|val| {
            asset_pool.load(Asset::new(*val));
        });
        
        let captures = asset_pool.capture();

        for i in 0..3 {
            assert_eq!(values[i], captures[i].value)
        }

        
    }
}

#[derive(PartialEq, Eq)]
pub struct AssetCapture {
    value: Decimal,
    idx: usize,
}

impl Ord for AssetCapture {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.value.cmp(&other.value)
    }
}

impl PartialOrd for AssetCapture {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub struct AccountCapture {
    asset_id: Vec<usize>,
    idx: usize
}

pub struct Account {
    idx: usize,
    asset_ids: Vec<usize>,
    asset_pool: Rc<AssetPool>
}

impl Account {
    pub fn new(idx: usize, asset_pool: Rc<AssetPool>) -> Account {
        Account { idx, asset_ids: vec![], asset_pool }
    }

    pub fn total_value(&self) -> Decimal {
        self.asset_ids
            .iter()
            .fold(Decimal::ZERO, |accum, next_id| {
                accum + self.asset_pool
                    .get(*next_id)
                    .unwrap_or(Decimal::ZERO)
            })
    }
}

pub struct Asset {
    value: Cell<Decimal>,
}

impl Asset {
    pub fn new(value: Decimal) -> Asset {
        Asset { value: Cell::new(value) }
    }

    pub fn mutate(&self, amount: Decimal) {
        self.value.set(self.value.get() + amount);
    }
}

pub struct AssetPool {
    assets: RefCell<Vec<Asset>>
}

impl AssetPool {
    pub fn new() -> Rc<AssetPool> {
        Rc::new(AssetPool { assets: RefCell::new(Vec::new()) })
    }

    pub fn load(&self, asset: Asset) -> usize {
        let mut assets = self.assets.borrow_mut();
        assets.push(asset);

        assets.len() - 1
    }

    pub fn get(&self, idx: usize) -> Option<Decimal> {
        if let Some(asset) = self.assets.borrow().get(idx) {
            Some(asset.value.get())
        } else { None }
    }

    pub unsafe fn get_unchecked(&self, idx: usize) -> Decimal {
        self.assets.borrow()
        .get_unchecked(idx)
        .value
        .get()
    }

    pub fn mutate(&self, idx: usize, change: Decimal) -> bool {
        if let Some(asset) = self.assets.borrow().get(idx) {
            asset.mutate(change);
            true
        } else { false }
    }

    pub unsafe fn mutate_unchecked(&self, idx: usize, change: Decimal) {
        self.assets.borrow().get_unchecked(idx).mutate(change)
    }

    /// Removes and returns the assets from the `AssetPool`.
    /// The `AssetPool`'s assets are replaced with an empty vector.
    pub fn unload(&self) -> Vec<Asset> {
        self.assets.replace(Vec::new())
    }

    /// Creates captures of all assets owned by the given `AssetPool`.
    /// The given `AssetPool` retains all of its assets.
    pub fn capture(&self) -> Vec<AssetCapture> {
        let assets = self.assets.borrow();
        let mut out = Vec::new();

        for idx in 0..assets.len() {
            out.push( AssetCapture { value: unsafe {
                assets.get_unchecked(idx).value.get()
            }, idx } );
        }

        out
    }

    /// Sorts the given captures by idx, and then converts all
    /// captures into assets, which are then given to the returned
    /// `AssetPool`.
    pub fn reload(mut captures: Vec<AssetCapture>) -> Rc<AssetPool> {
        captures.sort_unstable();

        AssetPool::reload_unchecked(captures)
    }

    /// Converts all captures into assets, which are then given
    /// to the returned `AssetPool`.
    /// 
    /// **Warning:** if captures are not sorted by idx, the returned
    /// `AssetPool` will not function properly
    pub fn reload_unchecked(captures: Vec<AssetCapture>) -> Rc<AssetPool> {
        let out = AssetPool::new();

        captures.into_iter().for_each(|cap| {
            out.load(Asset::new(cap.value));
        });

        out
    }
}

pub struct MutatorBaseCapture {
    total_change: Decimal,
    idx: usize
}

impl MutatorBaseCapture {
    fn capture(base: &MutatorBase) -> MutatorBaseCapture {
        MutatorBaseCapture { total_change: base.total_change, idx: base.idx }
    }
}

pub struct MutatorBase {
    pub idx: usize,
    pub target_idx: usize,
    pub change: Decimal,
    pub total_change: Decimal,
    pub is_add: bool,
    pub cycle: u32,
    cycle_reciprocal: f64,
    pub unix_reference: u64
}

impl MutatorBase {
    pub fn new(idx: usize, target_idx: usize, change: Decimal, 
        total_change: Decimal, is_add: bool, cycle: u32, unix_reference: u64) 
            -> MutatorBase 
    {
        let cycle_reciprocal = (1 as f64) / (cycle as f64);
        
        MutatorBase { idx, target_idx, change, total_change, is_add, cycle, cycle_reciprocal, unix_reference }
    }

    pub fn projection_length(&self, unix_initial_event: u64) -> u64 {
        
        ((unix_initial_event - (unix_initial_event % self.cycle as u64)) as f64 * self.cycle_reciprocal) as u64 + 1
    }

    pub fn unix_initial_event(&self, start: u64) -> u64 {
        let cycle64 = self.cycle as u64;
        let mut ur_cpy = self.unix_reference;
        let top = start + cycle64;
        let bottom = start - cycle64;

        if self.unix_reference != start {
            while cycle64 > top || cycle64 < bottom {
                ur_cpy += cycle64;
            }
        };

        if ur_cpy >= start {
            return ur_cpy;
        }

        ur_cpy + cycle64
    }
}

pub trait Mutator {
    fn on_event(&self, amount: Decimal) -> Decimal;
    fn capture(&self) -> String;
    fn reset(&mut self, capture: String);
}

pub struct EventMemento {
    time_pos: u64,
    account_states: Vec<AccountCapture>,
    mutator_states: Vec<String>
}

pub struct IntervalPoint {
    account_captures: Vec<AccountCapture>,
    mutator_captures: Vec<MutatorBaseCapture>,
    asset_captures: Vec<AssetCapture>
}