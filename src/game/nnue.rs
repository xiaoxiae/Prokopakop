use std::fs;
use std::path::Path;
use std::sync::OnceLock;

const HIDDEN_SIZE: usize = 128;
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

/// This is the quantised format that bullet outputs.
#[repr(C)]
pub struct Network {
    /// Column-Major `HIDDEN_SIZE x 768` matrix.
    /// Values have quantization of QA.
    feature_weights: [Accumulator; 768],
    /// Vector with dimension `HIDDEN_SIZE`.
    /// Values have quantization of QA.
    feature_bias: Accumulator,
    /// Column-Major `1 x (2 * HIDDEN_SIZE)`
    /// matrix, we use it like this to make the
    /// code nicer in `Network::evaluate`.
    /// Values have quantization of QB.
    output_weights: [i16; 2 * HIDDEN_SIZE],
    /// Scalar output bias.
    /// Value has quantization of QA * QB.
    output_bias: i16,
}

impl Network {
    /// Calculates the output of the network, starting from the already
    /// calculated hidden layer (done efficiently during makemoves).
    pub fn evaluate(&self, us: &Accumulator, them: &Accumulator) -> i32 {
        // Initialise output.
        let mut output = 0;

        // Side-To-Move Accumulator -> Output.
        for (&input, &weight) in us.vals.iter().zip(&self.output_weights[..HIDDEN_SIZE]) {
            output += screlu(input) * i32::from(weight);
        }

        // Not-Side-To-Move Accumulator -> Output.
        for (&input, &weight) in them.vals.iter().zip(&self.output_weights[HIDDEN_SIZE..]) {
            output += screlu(input) * i32::from(weight);
        }

        // Reduce quantization from QA * QA * QB to QA * QB.
        output /= i32::from(QA);

        // Add bias.
        output += i32::from(self.output_bias);

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
