use super::board::Move;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bound {
    Exact,
    Lower,
    Upper,
}

#[derive(Clone, Copy, Debug)]
pub struct TTEntry {
    pub key: u64,
    pub depth: usize,
    pub score: i32,
    pub bound: Bound,
    pub best_move: Option<Move>,
}

pub struct TranspositionTable {
    entries: Vec<Option<TTEntry>>,
}

impl TranspositionTable {
    /// `hash_mb` is the requested memory budget in MiB.
    pub fn new(hash_mb: usize) -> Self {
        let bytes = hash_mb.max(1).saturating_mul(1024 * 1024);

        let count = (bytes / std::mem::size_of::<Option<TTEntry>>()).max(1);

        Self {
            entries: vec![None; count],
        }
    }

    #[inline]
    fn index(&self, key: u64) -> usize {
        (key as usize) % self.entries.len()
    }

    #[inline]
    pub fn probe(&self, key: u64) -> Option<TTEntry> {
        self.entries[self.index(key)].filter(|entry| entry.key == key)
    }

    #[inline]
    pub fn store(&mut self, entry: TTEntry) {
        let index = self.index(entry.key);

        // Keep a deeper entry for the same position.
        let replace = match self.entries[index] {
            Some(old) if old.key == entry.key => {
                entry.depth >= old.depth || entry.bound == Bound::Exact
            }
            _ => true,
        };

        if replace {
            self.entries[index] = Some(entry);
        }
    }

    pub fn clear(&mut self) {
        self.entries.fill(None);
    }

    pub fn capacity(&self) -> usize {
        self.entries.len()
    }
}
