use std::fs;
use std::path::Path;
use std::sync::OnceLock;

const HIDDEN_SIZE: usize = 128;
const NUM_OUTPUT_BUCKETS: usize = 8;
const SCALE: i32 = 400;
const QA: i16 = 255;
const QB: i16 = 64;

static DEFAULT_NNUE: Network =
    unsafe { std::mem::transmute(*include_bytes!("../../data/nnue.bin")) };
static LOADED_NNUE: OnceLock<Box<Network>> = OnceLock::new();

#[inline]
/// Square Clipped ReLU - Activation Function.
/// Note that this takes the i16s in the accumulator to i32s.
/// Range is 0.0 .. 1.0 (in other words, 0 to QA*QA quantized).
fn screlu(x: i16) -> i32 {
    let y = i32::from(x).clamp(0, i32::from(QA));
    y * y
}

#[inline]
/// Calculate output bucket based on material count (all pieces on board).
/// Uses the same formula as Bullet's MaterialCount<8>:
///   bucket = (piece_count - 2) / divisor
/// where divisor = ceil(32 / NUM_BUCKETS) = 4 for 8 buckets
fn get_output_bucket(piece_count: u32) -> usize {
    const DIVISOR: u32 = 32u32.div_ceil(NUM_OUTPUT_BUCKETS as u32); // = 4
    ((piece_count.saturating_sub(2)) / DIVISOR).min(NUM_OUTPUT_BUCKETS as u32 - 1) as usize
}

/// This is the quantised format that bullet outputs.
#[repr(C)]
pub struct Network {
    /// Column-Major `HIDDEN_SIZE x 768` matrix.
    /// Values have quantization of QA.
    feature_weights: [Accumulator; 768],
    /// Vector with dimension `HIDDEN_SIZE`.
    /// Values have quantization of QA.
    feature_bias: Accumulator,
    /// Transposed layout: [bucket][hidden_neuron] for efficient bucket access.
    /// Shape: NUM_OUTPUT_BUCKETS rows x (2 * HIDDEN_SIZE) cols
    /// Values have quantization of QB.
    output_weights: [i16; NUM_OUTPUT_BUCKETS * 2 * HIDDEN_SIZE],
    /// One bias per bucket.
    /// Values have quantization of QA * QB.
    output_bias: [i16; NUM_OUTPUT_BUCKETS],
}

impl Network {
    /// Calculates the output of the network, starting from the already
    /// calculated hidden layer (done efficiently during makemoves).
    /// piece_count is the total number of pieces on the board for bucket selection.
    pub fn evaluate(&self, us: &Accumulator, them: &Accumulator, piece_count: u32) -> i32 {
        let bucket = get_output_bucket(piece_count);

        // With transposed weights, each bucket's weights are contiguous
        let bucket_weights_start = bucket * (2 * HIDDEN_SIZE);
        let bucket_weights =
            &self.output_weights[bucket_weights_start..bucket_weights_start + 2 * HIDDEN_SIZE];

        // Initialise output.
        let mut output = 0;

        // Side-To-Move Accumulator -> Output.
        for (&input, &weight) in us.vals.iter().zip(&bucket_weights[..HIDDEN_SIZE]) {
            output += screlu(input) * i32::from(weight);
        }

        // Not-Side-To-Move Accumulator -> Output.
        for (&input, &weight) in them.vals.iter().zip(&bucket_weights[HIDDEN_SIZE..]) {
            output += screlu(input) * i32::from(weight);
        }

        // Reduce quantization from QA * QA * QB to QA * QB.
        output /= i32::from(QA);

        // Add bias for this bucket.
        output += i32::from(self.output_bias[bucket]);

        // Apply eval scale.
        output *= SCALE;

        // Remove quantisation altogether.
        output /= i32::from(QA) * i32::from(QB);

        output
    }
}

/// A column of the feature-weights matrix.
/// Note the `align(64)`.
#[derive(Clone, Copy, Debug)]
#[repr(C, align(64))]
pub struct Accumulator {
    vals: [i16; HIDDEN_SIZE],
}

impl Accumulator {
    /// Initialised with bias so we can just efficiently
    /// operate on it afterwards.
    pub fn new(net: &Network) -> Self {
        net.feature_bias
    }

    /// Add a feature to an accumulator.
    pub fn add_feature(&mut self, feature_idx: usize, net: &Network) {
        for (i, d) in self
            .vals
            .iter_mut()
            .zip(&net.feature_weights[feature_idx].vals)
        {
            *i += *d
        }
    }

    /// Remove a feature from an accumulator.
    pub fn remove_feature(&mut self, feature_idx: usize, net: &Network) {
        for (i, d) in self
            .vals
            .iter_mut()
            .zip(&net.feature_weights[feature_idx].vals)
        {
            *i -= *d
        }
    }
}

/// Load a NNUE network from a file path.
/// Panics if the path is invalid or the network fails to load.
pub fn load_nnue_from_file(path: &Path) {
    // Fail if a network is already loaded
    if LOADED_NNUE.get().is_some() {
        panic!("NNUE network already loaded, please restart the engine.");
    }

    match fs::read(path) {
        Ok(data) => {
            if data.len() != std::mem::size_of::<Network>() {
                panic!(
                    "NNUE file size mismatch: expected {}, got {}",
                    std::mem::size_of::<Network>(),
                    data.len()
                );
            }

            // Create a boxed Network from the binary data
            let mut network = Box::new(unsafe { std::mem::zeroed::<Network>() });
            unsafe {
                std::ptr::copy_nonoverlapping(
                    data.as_ptr() as *const Network,
                    &mut *network as *mut Network,
                    1,
                );
            }

            let _ = LOADED_NNUE.get_or_init(|| network);
            println!("info string NNUE loaded successfully!");
        }
        Err(e) => {
            panic!("Failed to load NNUE file {}: {}", path.display(), e);
        }
    }
}

/// Get a reference to the active NNUE network.
/// If a network was loaded from file, returns that; otherwise returns the default.
pub fn get_network() -> &'static Network {
    LOADED_NNUE
        .get()
        .map(|boxed| &**boxed)
        .unwrap_or(&DEFAULT_NNUE)
}
