use std::{cell::{Cell, RefCell}, rc::Rc};

use rust_decimal::Decimal;

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use rust_decimal::Decimal;

    use crate::{AssetPool, Asset, MutatorPool, MutatorBase, StandardMutator, Modeller};

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

    struct MockModel {
        assets: Vec<Decimal>,
        partial_mutators: Vec<(Decimal, u64, u64)>,
    }

    impl MockModel {
        pub fn init() -> MockModel {
            MockModel {
                assets: vec![
                    Decimal::ZERO,
                    Decimal::new(1000, 0),
                    Decimal::new(-50000, 0)
                ],
                partial_mutators: vec![
                    (Decimal::new(100, 0), 3, 48),
                    (Decimal::new(5, 4), 4, 53),
                    (Decimal::new(50000, 0), 10, 41)
                ]
            }
        }
    }

    #[test]
    fn projection() {
        // Assets: 0, 1,000, -50,000
        // Mutators: (100, 3, 48), (0.0005, 4, 53), (50,000, 10, 41)
        // Start 50 End 69 (19 Days)

        let asset_pool = AssetPool::new();
        let mutator_pool = MutatorPool::new();
        
        let mock_model = MockModel::init();

        for i in 0..3 {
            let (change, cycle, urd) = mock_model.partial_mutators[i];
            let ai = asset_pool.load(Asset::new(mock_model.assets[0]));
            mutator_pool.load(
                Box::new(StandardMutator(MutatorBase::new(
                    i, ai, change, Decimal::ZERO, cycle, urd
                )))
            );
        } // In reality you'd probably map ids to your data
        
        let results = Modeller::new(
            Rc::clone(&asset_pool), 
            Rc::clone(&mutator_pool)
        ).project(50, 19, 1, 0, None);

        // Make some assertions with results and expected asset values.
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

    pub fn value_of_group(&self, idxs: Vec<usize>) -> Option<Decimal> {
        let assets = self.assets.borrow();
        let mut accum = Decimal::ZERO;
        
        for i in 0..idxs.len() {
            accum += assets[idxs[i]].get();

            if let Some(asset) = assets.get(idxs[i]) {
                accum += asset.get();
            } else { return None }
        }

        Some(accum)
    }

    pub unsafe fn value_of_group_unchecked(&self, idxs: Vec<usize>) -> Decimal {
        let assets = self.assets.borrow();
        let mut out = Decimal::ZERO;
        
        for i in 0..idxs.len() {
           out += assets.get_unchecked(idxs[i]).get(); 
        }

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
    pub cycle: u64,
    cycle_reciprocal: f64,
    pub unix_reference: u64
}

impl MutatorBase {
    pub fn new(idx: usize, target_idx: usize, change: Decimal, 
        total_change: Decimal, cycle: u64, unix_reference: u64) 
            -> MutatorBase 
    {
        let cycle_reciprocal = (1 as f64) / (cycle as f64);
        
        MutatorBase { idx, target_idx, change, total_change, cycle, cycle_reciprocal, unix_reference }
    }

    pub fn capture(&self) -> MutatorBaseCapture {
        MutatorBaseCapture { total_change: self.total_change, idx: self.idx }
    }

    pub fn reset(&mut self, capture: MutatorBaseCapture) {
        self.total_change = capture.total_change;
    }

    pub fn projection_length(&self, unix_initial_event: u64) -> u64 {
        // TODO: There's no way this is the best way to handle it.
        ((unix_initial_event - (unix_initial_event % self.cycle)) as f64 * self.cycle_reciprocal) as u64 + 1
    }

    pub fn unix_initial_event(&self, start: u64) -> u64 {
        let mut ur_cpy = self.unix_reference;
        let top = start + self.cycle;
        let bottom = start - self.cycle;

        if self.unix_reference != start {
            while self.cycle > top || self.cycle < bottom {
                ur_cpy += self.cycle;
            }
        };

        if ur_cpy < start {
            ur_cpy += self.cycle;
        }

        ur_cpy
    }
}

pub trait Mutator {
    fn on_event(&self, original_value: Decimal) -> Decimal;
    fn capture(&self) -> MutatorCapture;
    fn reset(&mut self, capture: MutatorCapture);
    fn borrow_base(&self) -> &MutatorBase;
    fn create_events(&self, start: u64, end: u64, idx: usize) -> Vec<Event>;
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

    fn create_events(&self, start: u64, end: u64, idx: usize) -> Vec<Event> {
        let mut out = Vec::new();

        if self.0.cycle > end - start || self.0.cycle == 0 {
            return out;
        }

        let uie = self.0.unix_initial_event(start);
        let rie = uie - start;
        let pl = self.0.projection_length(uie);

        for i in 0..pl {
            out.push(Event {
                time_pos:       rie + (self.0.cycle * i), 
                mutator_idx:    idx,
                asset_idx:      self.0.target_idx
            });
        }

        out
    }
}

pub struct MutatorPool {
    mutators: RefCell<Vec<Box<dyn Mutator>>>
}

impl MutatorPool {
    pub fn new() -> Rc<MutatorPool> {
        Rc::new(MutatorPool { mutators: RefCell::new(Vec::new()) })
    }

    /// Returns `None` if mutator is not found at `idx`.
    pub fn on_event(&self, idx: usize, asset_value: Decimal) -> Option<Decimal> {
        if let Some(out) = self.mutators.borrow().get(idx) {
            Some(out.on_event(asset_value))
        } else { None }
    }

    pub unsafe fn on_event_unchecked(&self, idx: usize, asset_value: Decimal) -> Decimal {
        self.mutators
            .borrow()
            .get_unchecked(idx)
            .on_event(asset_value)
    }

    pub fn load(&self, mutator: Box<dyn Mutator>) -> usize {
        let mut mutators = self.mutators.borrow_mut();
        mutators.push(mutator);
        mutators.len() - 1
    }
}

pub struct Event {
    time_pos: u64,
    mutator_idx: usize,
    asset_idx: usize
}

impl Event {
    pub fn new(time_pos: u64, mutator_idx: usize, asset_idx: usize) -> Event {
        Event { time_pos, mutator_idx, asset_idx }
    }

    pub fn trigger(&self, asset_pool: Rc<AssetPool>, mutator_pool: Rc<MutatorPool>) -> bool {
        let value = match asset_pool.get(self.asset_idx) {
            Some(value) => value,
            None => { return false; },
        };

        if let Some(new_value) = mutator_pool.on_event(self.mutator_idx, value) {
            asset_pool.mutate(self.asset_idx, new_value)
        } else { false }
    }

    pub unsafe fn trigger_unchecked(&self, asset_pool: Rc<AssetPool>, mutator_pool: Rc<MutatorPool>) {
        asset_pool.mutate_unchecked(self.asset_idx, 
            mutator_pool.on_event_unchecked(self.mutator_idx, 
                asset_pool.get_unchecked(self.asset_idx)
            )
        );
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
    mutator_states: Vec<MutatorCapture>,
    asset_captures: Vec<AssetCapture>
}

pub struct IntervalPoint {
    mutator_captures: Vec<MutatorCapture>, // Might only need MutatorBaseCapture
    asset_captures: Vec<AssetCapture>
}

pub struct ResultPacket {
    interval_points: Vec<IntervalPoint>,
    event_mementos: Vec<EventMemento>
}

pub struct Modeller {
    pub asset_pool: Rc<AssetPool>,
    pub mutator_pool: Rc<MutatorPool>,
    pub events: Vec<Event>
}

impl Modeller {
    pub fn new(asset_pool: Rc<AssetPool>, mutator_pool: Rc<MutatorPool>) -> Modeller {
        Modeller { asset_pool, mutator_pool, events: Vec::new() }
    }
    
    pub fn project(&self, start: u64, interval_len: u64, interval_count: u64, 
        interval_delay: u64, memento: Option<EventMemento>) -> ResultPacket 
    {
        todo!()
    }
}