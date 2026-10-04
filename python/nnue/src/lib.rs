use std::fs::File;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::thread::JoinHandle;

use memmap2::Mmap;
use numpy::{IntoPyArray, PyArrayMethods};
use pyo3::prelude::*;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;

const MAGIC: &[u8; 8] = b"NNUEBIN\0";

const VERSION: u32 = 2;
const HEADER_SIZE: usize = 32;
const RECORD_SIZE: usize = 32;

const OCCUPANCY_OFFSET: usize = 0;
const PIECES_OFFSET: usize = 8;
const SCORE_OFFSET: usize = 24;
const RESULT_OFFSET: usize = 26;
const FLAGS_OFFSET: usize = 27;

const FLAG_WHITE_TO_MOVE: u8 = 1;

const MAX_ACTIVE: usize = 32;

struct Dataset {
    mmap: Mmap,
    count: usize,
}

impl Dataset {
    fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = File::open(path)?;

        let size = file.metadata()?.len() as usize;

        if size < HEADER_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "dataset is too small to contain a header",
            ));
        }

        if (size - HEADER_SIZE) % RECORD_SIZE != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("dataset size is not aligned to {}-byte records", RECORD_SIZE),
            ));
        }

        let mmap = unsafe { Mmap::map(&file)? };
        // NOTE: no `advise(Advice::Random)` here. It switches off the OS read-ahead, so every first
        // touch of a page becomes its own 4 KB disk read. Chunks are read sequentially (see `touch`).

        let header = &mmap[..HEADER_SIZE];

        if &header[0..8] != MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid magic: expected {:?}, got {:?}", MAGIC, &header[0..8]),
            ));
        }

        let version = u32::from_le_bytes(header[8..12].try_into().unwrap());

        if version != VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported dataset version {}; expected {}", version, VERSION),
            ));
        }

        let record_size = u32::from_le_bytes(header[12..16].try_into().unwrap());

        if record_size as usize != RECORD_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported record size {}; expected {}", record_size, RECORD_SIZE),
            ));
        }

        let count = (size - HEADER_SIZE) / RECORD_SIZE;

        Ok(Self { mmap, count })
    }

    #[inline(always)]
    fn record(&self, index: usize) -> &[u8] {
        let start = HEADER_SIZE + index * RECORD_SIZE;
        &self.mmap[start..start + RECORD_SIZE]
    }

    fn touch(&self, start: usize, end: usize) {
        let from = HEADER_SIZE + start * RECORD_SIZE;
        let to = HEADER_SIZE + end * RECORD_SIZE;
        let bytes = &self.mmap[from..to];

        let mut acc = 0u8;

        for p in (0..bytes.len()).step_by(4096) {
            acc ^= bytes[p];
        }

        std::hint::black_box(acc);
    }
}

#[inline(always)]
fn read_i16(data: &[u8], offset: usize) -> i16 {
    i16::from_le_bytes([data[offset], data[offset + 1]])
}

/// Decode one v2 record into the same 64 sparse feature indices that the
/// old 136-byte format supplied:
///
///   idx[0..32]  = us
///   idx[32..64] = them
///
/// V2 stores the actual board compactly:
///   occupancy = occupied squares
///   pieces    = 4-bit side-to-move-relative piece codes
fn decode_v2_record(record: &[u8]) -> ([i16; 64], i16, f32, f32) {
    debug_assert_eq!(record.len(), RECORD_SIZE);

    let occupancy = u64::from_le_bytes(
        record[OCCUPANCY_OFFSET..OCCUPANCY_OFFSET + 8]
            .try_into()
            .unwrap(),
    );

    let score = read_i16(record, SCORE_OFFSET);

    let result_code = record[RESULT_OFFSET];
    debug_assert!(result_code <= 2);

    let flags = record[FLAGS_OFFSET];
    let white_to_move = flags & FLAG_WHITE_TO_MOVE != 0;

    let mut us = [768i16; MAX_ACTIVE];
    let mut them = [768i16; MAX_ACTIVE];

    let mut n = 0usize;
    let mut occupied = occupancy;

    while occupied != 0 {
        let sq = occupied.trailing_zeros() as usize;
        occupied &= occupied - 1;

        // Pieces are packed in increasing-square order.
        let nibble = n;
        let byte = record[PIECES_OFFSET + nibble / 2];

        let code = if nibble & 1 == 0 {
            byte & 0x0f
        } else {
            byte >> 4
        };

        debug_assert!(code < 12);

        let own = code < 6;
        let pt = (code % 6) as usize;

        let us_sq = if white_to_move {
            sq
        } else {
            sq ^ 56
        };

        let them_sq = if white_to_move {
            sq ^ 56
        } else {
            sq
        };

        us[n] = ((if own { 0 } else { 384 })
            + pt * 64
            + us_sq) as i16;

        them[n] = ((if own { 384 } else { 0 })
            + pt * 64
            + them_sq) as i16;

        n += 1;
    }

    // Preserve the old feature ordering. EmbeddingBag doesn't require this,
    // but it keeps the representation deterministic and identical to the
    // previous pos_features() ordering.
    us[..n].sort_unstable();
    them[..n].sort_unstable();

    let mut idx = [0i16; 64];
    idx[..32].copy_from_slice(&us);
    idx[32..64].copy_from_slice(&them);

    let result = result_code as f32;

    (
        idx,
        score,
        result,
        if white_to_move { 1.0 } else { 0.0 },
    )
}

/// Gather one batch from v2 records.
fn build_batch(ds: &Dataset, base: usize, rel: &[usize]) -> (Vec<i16>, Vec<f32>, Vec<f32>) {
    let bs = rel.len();

    let mut idx = Vec::with_capacity(bs * 2 * MAX_ACTIVE);
    let mut scores = Vec::with_capacity(bs);
    let mut results = Vec::with_capacity(bs);

    for &r in rel {
        let record = ds.record(base + r);

        let (features, score, result, _stm) = decode_v2_record(record);

        idx.extend_from_slice(&features);
        scores.push(score as f32);
        results.push(result);
    }

    (idx, scores, results)
}

#[pyclass]
struct BatchIterator {
    dataset: Arc<Dataset>,
    total: usize,
    batch: usize,
    shuffle_size: usize,

    chunk_start: usize,
    perm: Vec<usize>,
    batch_pos: usize,

    /// Background thread that is reading the NEXT chunk into the page cache.
    prefetch: Option<JoinHandle<()>>,

    rng: StdRng,
}

impl BatchIterator {
    // Not part of the Python API (kept out of #[pymethods]).
    fn prepare_chunk(&mut self, n: usize) {
        self.perm.clear();
        self.perm.extend(0..n);
        self.perm.shuffle(&mut self.rng);
        self.batch_pos = 0;
    }
}

#[pymethods]
impl BatchIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        loop {
            if slf.batch_pos < slf.perm.len() {
                let start = slf.batch_pos;
                let end = (start + slf.batch).min(slf.perm.len());
                let bs = end - start;

                let ds = Arc::clone(&slf.dataset);
                let base = slf.chunk_start;
                let rel: Vec<usize> = slf.perm[start..end].to_vec();

                // Build the batch WITHOUT holding the GIL. Reading the memory map can block on
                // disk; while the GIL is held, the training thread cannot run Python at all.
                // (`allow_threads` was renamed `detach` in pyo3 0.26+.)
                let (idx, scores, results) = py.detach(move || build_batch(&ds, base, &rel));

                slf.batch_pos = end;

                if slf.batch_pos == slf.perm.len() {
                    slf.chunk_start += slf.perm.len();
                }

                let idx = idx.into_pyarray(py).reshape([bs, 2, MAX_ACTIVE])?;
                let scores = scores.into_pyarray(py);
                let results = results.into_pyarray(py);

                return Ok(Some((idx, scores, results).into_pyobject(py)?.unbind().into_any()));
            }

            // Need a new chunk.
            if slf.chunk_start >= slf.total {
                return Ok(None);
            }

            let chunk_start = slf.chunk_start;
            let chunk_end = chunk_start.saturating_add(slf.shuffle_size).min(slf.total);
            let n = chunk_end - chunk_start;

            // Make sure this chunk is in the page cache: wait for the prefetch thread started during
            // the previous chunk, or (first chunk) read it now. Neither holds the GIL.
            let ds = Arc::clone(&slf.dataset);
            let pending = slf.prefetch.take();
            py.detach(move || match pending {
                Some(handle) => {
                    let _ = handle.join();
                }
                None => ds.touch(chunk_start, chunk_end),
            });

            // Start reading the NEXT chunk in the background while we train on this one.
            let next_start = chunk_end;
            if next_start < slf.total {
                let next_end = next_start.saturating_add(slf.shuffle_size).min(slf.total);
                let ds_next = Arc::clone(&slf.dataset);
                slf.prefetch = Some(std::thread::spawn(move || ds_next.touch(next_start, next_end)));
            }

            slf.prepare_chunk(n);
        }
    }
}

#[pyfunction]
#[pyo3(signature = (
    path,
    batch=16384,
    shuffle_size=1_000_000,
    seed=0,
    max_positions=0,
))]
fn iter_batches(
    path: &str,
    batch: usize,
    shuffle_size: usize,
    seed: u64,
    max_positions: usize,
) -> PyResult<BatchIterator> {
    if batch == 0 {
        return Err(pyo3::exceptions::PyValueError::new_err("batch must be greater than 0"));
    }

    if shuffle_size == 0 {
        return Err(pyo3::exceptions::PyValueError::new_err("shuffle_size must be greater than 0"));
    }

    let dataset = Dataset::open(path)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

    let total = if max_positions > 0 {
        max_positions.min(dataset.count)
    } else {
        dataset.count
    };

    Ok(BatchIterator {
        dataset: Arc::new(dataset),
        total,
        batch,
        shuffle_size,
        chunk_start: 0,
        perm: Vec::new(),
        batch_pos: 0,
        prefetch: None,
        rng: StdRng::seed_from_u64(seed),
    })
}

#[pymodule]
fn fastdata(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<BatchIterator>()?;
    m.add_function(wrap_pyfunction!(iter_batches, m)?)?;
    Ok(())
}
