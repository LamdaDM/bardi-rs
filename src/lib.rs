use std::{cell::{Cell, RefCell}, rc::Rc, borrow::{Borrow, BorrowMut}, slice::SliceIndex};

use rust_decimal::Decimal;

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use crate::{AssetPool, Asset};

    #[test]
    fn asset_pool_changes() {
        let asset_pool = AssetPool::new();

        let idx = asset_pool.load(Asset::new(Decimal::new(5090, 2)));
        
        unsafe {
            let new_amount = asset_pool.get_unchecked(idx) + Decimal::new(-9, 1);
            asset_pool.mutate_unchecked(idx, new_amount);
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

pub struct AccountValueCapture {
    total_value: Decimal,
    idx: usize
}

pub struct AccountCapture {
    asset_ids: Vec<usize>,
    idx: usize
}

pub struct Account {
    asset_ids: Vec<usize>
}

impl Account {
    pub fn new() -> Account {
        Account { asset_ids: vec![] }
    }

    pub fn get_asset_ids(&self) -> Vec<usize> {
        self.asset_ids.clone()
    }

    pub fn add_asset(&mut self, asset_idx: usize) -> usize {
        self.asset_ids.push(asset_idx);

        self.asset_ids.len() - 1
    }

    pub fn total_value(&self, asset_pool: Rc<AssetPool>) -> Decimal {
        self.asset_ids
            .iter()
            .fold(Decimal::ZERO, |accum, next_id| {
                accum + asset_pool
                    .get(*next_id)
                    .unwrap_or(Decimal::ZERO)
            })
    }
}

pub struct AccountPool {
    accounts: RefCell<Vec<Account>>,
    asset_pool: Rc<AssetPool>
}

impl AccountPool {
    pub fn new(asset_pool: Rc<AssetPool>) -> Rc<AccountPool> {
        Rc::new(AccountPool { accounts: RefCell::new(vec![]), asset_pool })
    }

    pub fn load_account(&self) -> usize {
        let mut accounts = self.accounts.borrow_mut();
        accounts.push(Account::new());
        accounts.len() - 1
    }

    pub fn load_into_account(&self, asset_idx: usize, account_idx: usize) -> Option<usize> {
        if let Some(acc) = self.accounts.borrow_mut().get_mut(account_idx) {
            Some(acc.add_asset(asset_idx))
        } else { None }
    }

    pub unsafe fn load_into_account_unchecked(&self, asset_idx: usize, account_idx: usize) -> usize {
        self.accounts
            .borrow_mut()
            .get_unchecked_mut(account_idx)
            .add_asset(asset_idx)
    }

    /// Get total value of the account at idx.
    /// 
    /// Returns None if no element is found at idx.
    pub fn get(&self, idx: usize) -> Option<Decimal> {
        if let Some(account) = self.accounts.borrow().get(idx) {
            Some(account.total_value(Rc::clone(&self.asset_pool)))
        } else { None }
    }

    /// Get total value of the account at idx.
    /// 
    /// No out of bounds checking, which may result in undefined behavior if no element is found at idx.
    pub unsafe fn get_unchecked(&self, idx: usize) -> Decimal {
        self.accounts
            .borrow()
            .get_unchecked(idx)
            .total_value(Rc::clone(&self.asset_pool))
    }

    /// Removes and returns the accounts from the given `AccountPool`.
    /// 
    /// The accounts are replaced with an empty vector.`
    pub fn unload(&self) -> Vec<Account> {
        self.accounts.replace(Vec::new())
    }
}

pub struct Asset {
    value: Cell<Decimal>,
}

impl Asset {
    pub fn new(value: Decimal) -> Asset {
        Asset { value: Cell::new(value) }
    }

    pub fn get(&self) -> Decimal {
        self.value.get()
    }

    /// Sets the inner value to the new amount given.
    /// 
    /// Addition operations can be done such as:
    /// 
    /// ``` rust
    /// let asset = Asset::new(Decimal::new(1000, 2));
    /// 
    /// asset.mutate(asset.get() + Decimal::new(100, 1))
    /// ```
    pub fn mutate(&self, amount: Decimal) {
        self.value.set(amount);
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

    /// Calls `mutate` on the asset found at `idx`, which sets the asset's value to the given `change`.
    /// 
    /// Returns true if asset was found.
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
    /// 
    /// The assets are replaced with an empty vector.
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

    pub fn capture(&self) -> MutatorBaseCapture {
        MutatorBaseCapture { total_change: self.total_change, idx: self.idx }
    }

    pub fn reset(&mut self, capture: MutatorBaseCapture) {
        self.total_change = capture.total_change;
    }

    pub fn projection_length(&self, unix_initial_event: u64) -> u64 {
        // TODO: There's no way this is the best way to handle it.
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
    fn on_event(&self, original_value: Decimal) -> Decimal;
    fn capture(&self) -> MutatorCapture;
    fn reset(&mut self, capture: MutatorCapture);
    fn borrow_base(&self) -> &MutatorBase;
}

pub struct MutatorCapture {
    base: MutatorBaseCapture,
    variant: String
}

/// Default mutator which only uses data in MutatorBase.
pub struct StandardMutator(MutatorBase);

impl Mutator for StandardMutator {
    fn on_event(&self, ov: Decimal) -> Decimal {
        ov + self.0.change
    }

    fn capture(&self) -> MutatorCapture {
        MutatorCapture { base: self.0.capture(), variant: String::new() }
    }

    fn borrow_base(&self) -> &MutatorBase {
        &self.0
    }

    fn reset(&mut self, capture: MutatorCapture) {
        self.0.reset(capture.base)
    }
}

pub struct MutatorPool {
    mutators: RefCell<Vec<Box<dyn Mutator>>>
}

impl MutatorPool {
    pub fn new() -> Rc<MutatorPool> {
        Rc::new(MutatorPool { mutators: RefCell::new(Vec::new()) })
    }

    pub fn on_event(&self, idx: usize, asset_value: Decimal) -> Decimal {
        unsafe {
            self.mutators.borrow().get_unchecked(idx).on_event(asset_value)
        }
    }
}

pub struct Event {
    time_pos: u64,
    pool: Rc<MutatorPool>,
    mutator_idx: usize
}

impl Event {
    pub fn trigger(&self, asset_pool: Rc<AssetPool>, asset_idx: usize) {
        let value = unsafe {
            asset_pool.get_unchecked(asset_idx) 
        };

        let change = self.pool.on_event(self.mutator_idx, value);

        if !asset_pool.mutate(asset_idx, change) {
            panic!("Unable to mutate asset {}!", asset_idx);
        }
    }
}

impl Ord for Event {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.time_pos.cmp(&other.time_pos)
    }
}

impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for Event {}

impl PartialEq for Event {
    fn eq(&self, other: &Self) -> bool {
        self.time_pos == other.time_pos && self.mutator_idx == other.mutator_idx
    }
}

pub struct EventMemento {
    time_pos: u64,
    account_states: Vec<AccountCapture>,
    mutator_states: Vec<MutatorCapture>,
    asset_captures: Vec<AssetCapture>
}

pub struct IntervalPoint {
    account_value_captures: Vec<AccountValueCapture>,
    mutator_captures: Vec<MutatorCapture>, // Might only need MutatorBaseCapture
    asset_captures: Vec<AssetCapture>
}

pub struct Modeller {
    pub asset_pool: Rc<AssetPool>,
    pub mutator_pool: Rc<MutatorPool>,
    pub account_pool: Rc<AccountPool>
}