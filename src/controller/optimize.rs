//! SPSA parameter optimizer for chess engine tuning.
//!
//! Modifies `src/engine/search/params.rs`, builds perturbed binaries, runs tournaments
//! via fastchess, and updates parameters using gradient estimation.
//!
//! See <https://www.chessprogramming.org/SPSA>.

use rand::Rng;
use regex::Regex;
use std::fmt;
use std::fs;
use std::io::{self, Write as IoWrite};
use std::path::{Path, PathBuf};
use std::process::Command;

const PARAMS_FILE: &str = "src/engine/search/params.rs";
const BINARY_DIR: &str = "target/release";
const FASTCHESS_PATH: &str = "./bin/fastchess/fastchess";
const OPENING_BOOK: &str = "data/book.pgn";

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for the SPSA optimizer.
///
/// Each iteration, the gain sequences are computed as:
///   a_k = a / (k + 1 + A)^alpha
///   c_k = c / (k + 1)^gamma
/// where A = a_ratio * iterations.
///
/// Parameters are perturbed by ±c_k and the gradient is estimated from
/// tournament results, then applied with step size a_k.
#[derive(Debug, Clone)]
pub struct OptimizeConfig {
    pub iterations: u32,
    pub time_control: String,
    pub concurrency: u32,

    /// How far parameters move per iteration.
    pub a: f64,
    /// Size of parameter perturbations.
    pub c: f64,

    /// Decay rate for a. 0.602
    pub alpha: f64,
    /// Decay rate for c. 0.101
    pub gamma: f64,
    /// Delays the decay of a_k in early iterations.
    pub a_ratio: f64,
}

impl Default for OptimizeConfig {
    fn default() -> Self {
        Self {
            iterations: 100,
            time_control: "10+0.1".to_string(),
            concurrency: 8,
            a: 0.1,
            c: 0.05,
            alpha: 0.602,
            gamma: 0.101,
            a_ratio: 0.1,
        }
    }
}

// ============================================================================
// Tunable Parameter
// ============================================================================

#[derive(Debug, Clone)]
pub struct TunableParameter {
    pub name: String,
    pub value: f64,
    pub min_val: f64,
    pub max_val: f64,
    pub is_int: bool,
}

impl TunableParameter {
    fn normalized(&self) -> f64 {
        (self.value - self.min_val) / (self.max_val - self.min_val)
    }

    fn denormalize(&self, norm: f64) -> f64 {
        self.min_val + norm * (self.max_val - self.min_val)
    }

    fn with_value(&self, value: f64) -> Self {
        Self {
            name: self.name.clone(),
            value: if self.is_int { value.round() } else { value },
            min_val: self.min_val,
            max_val: self.max_val,
            is_int: self.is_int,
        }
    }

    fn perturb(&self, delta: f64, c_k: f64, direction: f64) -> Self {
        let norm = (self.normalized() + direction * c_k * delta).clamp(0.0, 1.0);
        self.with_value(self.denormalize(norm))
    }

    fn update(&self, gradient: f64, a_k: f64) -> Self {
        let new_norm = (self.normalized() + a_k * gradient).clamp(0.0, 1.0);
        self.with_value(self.denormalize(new_norm))
    }
}

impl fmt::Display for TunableParameter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_int {
            write!(f, "{}", self.value as i64)
        } else {
            write!(f, "{:.1}", self.value)
        }
    }
}

// ============================================================================
// SPSA Optimizer
// ============================================================================

pub struct SpsaOptimizer {
    config: OptimizeConfig,
    params: Vec<TunableParameter>,
    original_content: String,
}

impl SpsaOptimizer {
    pub fn new(config: OptimizeConfig) -> Result<Self, String> {
        if !Path::new(FASTCHESS_PATH).exists() {
            return Err(format!("fastchess not found at {}", FASTCHESS_PATH));
        }
        if !Path::new(OPENING_BOOK).exists() {
            return Err(format!("Opening book not found at {}", OPENING_BOOK));
        }

        let original_content = fs::read_to_string(PARAMS_FILE)
            .map_err(|e| format!("Failed to read {}: {}", PARAMS_FILE, e))?;

        let params = parse_params_file(&original_content)?;
        if params.is_empty() {
            return Err(format!("No tunable parameters found in {}", PARAMS_FILE));
        }

        Ok(Self {
            config,
            params,
            original_content,
        })
    }

    pub fn optimize(&mut self) -> Result<(), String> {
        self.print_parameters();

        let big_a = self.config.a_ratio * self.config.iterations as f64;

        for i in 0..self.config.iterations {
            eprintln!("Iteration {}/{}", i + 1, self.config.iterations);
            self.params = self.spsa_iteration(i, big_a)?;
            self.log_iteration();
        }

        self.print_final_results();
        self.prompt_apply();
        Ok(())
    }

    fn spsa_iteration(&self, iteration: u32, big_a: f64) -> Result<Vec<TunableParameter>, String> {
        // https://www.chessprogramming.org/SPSA#Automated_Tuning
        let k = (iteration + 1) as f64;
        let a_k = self.config.a / (big_a + k).powf(self.config.alpha);
        let c_k = self.config.c / k.powf(self.config.gamma);

        // Bernoulli +/-1 perturbations
        let mut rng = rand::rng();
        let delta: Vec<f64> = (0..self.params.len())
            .map(|_| if rng.random_bool(0.5) { 1.0 } else { -1.0 })
            .collect();

        // Build perturbed binaries
        let params_plus: Vec<_> = self
            .params
            .iter()
            .zip(&delta)
            .map(|(p, &d)| p.perturb(d, c_k, 1.0))
            .collect();
        let params_minus: Vec<_> = self
            .params
            .iter()
            .zip(&delta)
            .map(|(p, &d)| p.perturb(d, c_k, -1.0))
            .collect();

        let binary_plus = self.build_variant(&params_plus, &format!("spsa-plus-{}", iteration))?;
        let binary_minus =
            self.build_variant(&params_minus, &format!("spsa-minus-{}", iteration))?;

        // Run tournament
        let games = self.config.concurrency * 4;
        let (score_plus, score_minus) = run_tournament(
            &binary_plus,
            &binary_minus,
            games,
            &self.config.time_control,
            self.config.concurrency,
        )?;

        // Gradient estimation
        let total = score_plus + score_minus;
        let score_diff = if total > 0.0 {
            (score_plus - score_minus) / total
        } else {
            0.0
        };
        eprintln!(
            "  Score: +{:.1} / -{:.1} (diff: {:+.3})",
            score_plus, score_minus, score_diff
        );

        // Update parameters
        Ok(self
            .params
            .iter()
            .zip(&delta)
            .map(|(param, &d)| {
                let gradient = score_diff / (2.0 * c_k * d);
                param.update(gradient, a_k)
            })
            .collect())
    }

    fn build_variant(&self, params: &[TunableParameter], name: &str) -> Result<PathBuf, String> {
        let content = update_params_content(params, &self.original_content);
        fs::write(PARAMS_FILE, &content)
            .map_err(|e| format!("Failed to write {}: {}", PARAMS_FILE, e))?;
        build_binary(name)
    }

    fn log_iteration(&self) {
        for p in &self.params {
            eprintln!("    {}: {}", p.name, p);
        }
        eprintln!();
    }

    fn print_parameters(&self) {
        eprintln!("Found {} tunable parameters:", self.params.len());
        for p in &self.params {
            eprintln!(
                "  {}: {} (range: {}-{})",
                p.name, p.value, p.min_val, p.max_val
            );
        }
        eprintln!();
    }

    fn print_final_results(&self) {
        eprintln!("{}", "=".repeat(50));
        eprintln!("Final tuned parameters:");
        eprintln!("{}", "=".repeat(50));
        for p in &self.params {
            let type_str = if p.is_int { "usize" } else { "f32" };
            eprintln!("pub const {}: {} = {};", p.name, type_str, p);
        }
        eprintln!();
    }

    fn prompt_apply(&mut self) {
        eprint!("Apply these parameters to params.rs? [y/N]: ");
        io::stderr().flush().ok();
        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_ok() {
            if input.trim().eq_ignore_ascii_case("y") || input.trim().eq_ignore_ascii_case("yes") {
                self.original_content = update_params_content(&self.params, &self.original_content);
                eprintln!("Parameters applied to params.rs");
            }
        }
    }
}

impl Drop for SpsaOptimizer {
    fn drop(&mut self) {
        // Restore original params.rs
        if let Err(e) = fs::write(PARAMS_FILE, &self.original_content) {
            eprintln!("Warning: Failed to restore {}: {}", PARAMS_FILE, e);
        }

        // Clean up temporary binaries
        if let Ok(entries) = fs::read_dir(BINARY_DIR) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with("prokopakop-spsa-") {
                        let _ = fs::remove_file(entry.path());
                    }
                }
            }
        }
    }
}

// ============================================================================
// File Parsing & Writing
// ============================================================================

fn parse_params_file(content: &str) -> Result<Vec<TunableParameter>, String> {
    let re = Regex::new(
        r"pub const (\w+): (f32|usize) = ([\d.]+);.*?//.*?min:\s*([\d.]+),\s*max:\s*([\d.]+)",
    )
    .map_err(|e| e.to_string())?;

    re.captures_iter(content)
        .map(|cap| {
            Ok(TunableParameter {
                name: cap[1].to_string(),
                value: cap[3].parse().map_err(|e| format!("Parse error: {}", e))?,
                min_val: cap[4].parse().map_err(|e| format!("Parse error: {}", e))?,
                max_val: cap[5].parse().map_err(|e| format!("Parse error: {}", e))?,
                is_int: &cap[2] == "usize",
            })
        })
        .collect()
}

fn update_params_content(params: &[TunableParameter], content: &str) -> String {
    let mut result = content.to_string();
    for param in params {
        let pattern = format!(r"(pub const {}: (?:f32|usize) = )([\d.]+)(;)", param.name);
        let re = Regex::new(&pattern).unwrap();
        result = re
            .replace(&result, format!("${{1}}{}${{3}}", param))
            .to_string();
    }
    result
}

// ============================================================================
// Build & Tournament
// ============================================================================

fn build_binary(name: &str) -> Result<PathBuf, String> {
    eprintln!("  Building {}...", name);

    let output = Command::new("cargo")
        .args(["build", "--release"])
        .output()
        .map_err(|e| format!("Failed to run cargo: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "Build failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let src = Path::new(BINARY_DIR).join("prokopakop");
    let dst = Path::new(BINARY_DIR).join(format!("prokopakop-{}", name));
    fs::copy(&src, &dst).map_err(|e| format!("Failed to copy binary: {}", e))?;

    Ok(dst)
}

fn run_tournament(
    binary_plus: &Path,
    binary_minus: &Path,
    games: u32,
    tc: &str,
    concurrency: u32,
) -> Result<(f64, f64), String> {
    eprintln!("  Running {} games...", games);

    let results_file = Path::new("/tmp/spsa_results.json");
    let _ = fs::remove_file(results_file);

    let output = Command::new(FASTCHESS_PATH)
        .args([
            "-engine",
            &format!("cmd={}", binary_plus.display()),
            "name=plus",
            "-engine",
            &format!("cmd={}", binary_minus.display()),
            "name=minus",
            "-each",
            &format!("tc={}", tc),
            "restart=on",
            "-rounds",
            &(games / 2).to_string(),
            "-concurrency",
            &concurrency.to_string(),
            "-openings",
            &format!("file={}", OPENING_BOOK),
            "format=pgn",
            "plies=6",
            "order=random",
            "-pgnout",
            "file=/tmp/spsa_games.pgn",
            "-config",
            "outname=/tmp/spsa_results.json",
            "-recover",
        ])
        .output()
        .map_err(|e| format!("Failed to run fastchess: {}", e))?;

    if !output.status.success() {
        eprintln!(
            "Warning: fastchess error: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let data: serde_json::Value = fs::read_to_string(results_file)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    let stats = &data["stats"];
    let plus_vs_minus = &stats["plus vs minus"];
    let minus_vs_plus = &stats["minus vs plus"];

    let get_u64 = |v: &serde_json::Value, key: &str| v[key].as_u64().unwrap_or(0);

    let wins_plus = get_u64(plus_vs_minus, "wins") + get_u64(minus_vs_plus, "losses");
    let wins_minus = get_u64(minus_vs_plus, "wins") + get_u64(plus_vs_minus, "losses");
    let draws = get_u64(plus_vs_minus, "draws") + get_u64(minus_vs_plus, "draws");

    Ok((
        wins_plus as f64 + 0.5 * draws as f64,
        wins_minus as f64 + 0.5 * draws as f64,
    ))
}

// ============================================================================
// Entry Point
// ============================================================================

pub fn run_optimizer(config: OptimizeConfig) {
    match SpsaOptimizer::new(config) {
        Ok(mut optimizer) => {
            if let Err(e) = optimizer.optimize() {
                eprintln!("Optimization failed: {}", e);
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Failed to initialize optimizer: {}", e);
            std::process::exit(1);
        }
    }
}
