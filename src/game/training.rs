use crate::controller::game_controller::GameController;
use crate::game::pieces::Color;
use fxhash::FxHashMap;
use rand::Rng;
use rayon::prelude::*;
use std::fs::File;
use std::io::Write;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

/// Represents a single training position with evaluation and game result
#[derive(Debug, Clone)]
pub struct TrainingPosition {
    pub fen: String,
    pub zobrist_key: u64,
    pub evaluation: f32, // White-relative, in centipawns
    pub result: f32,     // White-relative (1.0 = white win, 0.5 = draw, 0.0 = white loss)
}

impl TrainingPosition {
    pub fn to_line(&self) -> String {
        format!(
            "{} | {} | {}",
            self.fen, self.evaluation as i32, self.result
        )
    }
}

/// Represents the result of a game
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GameResult {
    WhiteWin,
    Draw,
    BlackWin,
}

impl GameResult {
    /// Convert to white-relative score
    pub fn to_white_score(self) -> f32 {
        match self {
            GameResult::WhiteWin => 1.0,
            GameResult::Draw => 0.5,
            GameResult::BlackWin => 0.0,
        }
    }
}

/// Configuration for training data generation
#[derive(Debug, Clone)]
pub struct TrainingConfig {
    pub num_games: u32,
    pub search_depth: usize,
    pub start_moves_min: u32,
    pub start_moves_max: u32,
}

impl TrainingConfig {
    pub fn new(
        num_games: u32,
        search_depth: usize,
        start_moves_min: u32,
        start_moves_max: u32,
    ) -> Self {
        Self {
            num_games,
            search_depth,
            start_moves_min,
            start_moves_max,
        }
    }

    pub fn random_starting_moves(&self) -> u32 {
        let mut rng = rand::rng();
        rng.random_range(self.start_moves_min..=self.start_moves_max)
    }
}

/// Generates training data through self-play with parallel game execution
pub struct TrainingDataGenerator {
    config: TrainingConfig,
}

impl TrainingDataGenerator {
    pub fn new(config: TrainingConfig) -> Self {
        Self { config }
    }

    /// Play a single game and collect training data
    fn play_game(&self) -> Vec<TrainingPosition> {
        let mut controller = GameController::new();
        controller.initialize();
        controller.move_overhead = 0;

        let mut positions = Vec::new();

        let mut game_result = None;

        // Play random starting moves before collecting training data
        let num_starting_moves = self.config.random_starting_moves();
        for _ in 0..num_starting_moves {
            let (move_count, moves_array) = controller.game.get_moves();
            if move_count == 0 {
                break; // Game ended during random moves
            }

            let mut rng = rand::rng();
            let random_idx = rng.random_range(0..move_count as usize);
            let selected_move = moves_array[random_idx];
            controller.game.make_move(selected_move);
            controller
                .position_history
                .push(controller.game.zobrist_key);
        }

        // Play until game ends or max halfmoves reached
        loop {
            if controller.game.is_fifty_move_rule() {
                game_result = Some(GameResult::Draw);
                break;
            }

            if controller
                .position_history
                .is_threefold_repetition(controller.game.zobrist_key)
            {
                game_result = Some(GameResult::Draw);
                break;
            }

            // Store current position before search
            let current_fen = controller.game.get_fen();

            let search_params = vec!["depth".to_string(), self.config.search_depth.to_string()];
            controller.search(search_params, false);

            // Wait for search to complete naturally (don't interrupt)
            let search_result = controller.wait_for_search();

            if let Some(result) = search_result {
                // Check if move is valid (not empty)
                if result.best_move != 0 {
                    positions.push(TrainingPosition {
                        fen: current_fen,
                        zobrist_key: controller.game.zobrist_key,
                        evaluation: match controller.game.side {
                            Color::White => result.evaluation,
                            Color::Black => -result.evaluation,
                        },
                        result: 0.0, // Will be set after determining game result
                    });

                    // Make the best move
                    controller.game.make_move(result.best_move);
                    controller
                        .position_history
                        .push(controller.game.zobrist_key);
                } else {
                    // No move found - likely checkmate or stalemate
                    break;
                }
            } else {
                break;
            }
        }

        // Determine game result
        if game_result.is_none() {
            game_result = Some(determine_game_result(&controller));
        }

        // Set the result for all positions now that we know the final result
        let final_result = game_result.unwrap().to_white_score();
        for pos in &mut positions {
            pos.result = final_result;
        }

        positions
    }

    /// Generate all training data with parallel game playing and immediate file writes
    pub fn generate_parallel_to_file(&self, path: &str) -> std::io::Result<u64> {
        let start_time = Instant::now();

        println!(
            "Generating training data for {} games in parallel...",
            self.config.num_games
        );

        // Create channel for sending training positions from worker threads to writer thread
        let (sender, receiver) = mpsc::channel::<Vec<TrainingPosition>>();
        let path = path.to_string();

        // Spawn writer thread that immediately writes positions to file
        let writer_thread = thread::spawn(move || {
            let mut file = File::create(&path)?;
            let mut total_positions = 0u64;
            let mut games_processed = 0u32;
            let mut unique_positions = FxHashMap::default();
            let writer_start_time = Instant::now();

            for positions_batch in receiver {
                for pos in positions_batch {
                    writeln!(file, "{}", pos.to_line())?;
                    unique_positions.insert(pos.zobrist_key, ());
                    total_positions += 1;
                }
                games_processed += 1;

                if games_processed % 10 == 0 {
                    let elapsed = writer_start_time.elapsed();
                    let duration_secs = elapsed.as_secs_f64();
                    let positions_per_sec = total_positions as f64 / duration_secs;
                    let unique_count = unique_positions.len() as f64;
                    let uniqueness_pct = (unique_count / total_positions as f64) * 100.0;
                    println!(
                        "Completed {} games ({} positions written, {:.2} positions/sec, {:.2}% unique)",
                        games_processed, total_positions, positions_per_sec, uniqueness_pct
                    );
                }
            }

            Ok::<u64, std::io::Error>(total_positions)
        });

        (1..=self.config.num_games).into_par_iter().for_each_with(
            sender.clone(),
            |tx, _game_num| {
                let _ = tx.send(self.play_game());
            },
        );

        // Drop the original sender so the writer thread knows all games are done
        drop(sender);

        // Wait for writer thread to complete and get total position count
        let total_positions = match writer_thread.join() {
            Ok(result) => result?,
            Err(_) => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Writer thread panicked",
            ))?,
        };

        let elapsed = start_time.elapsed();
        let duration_secs = elapsed.as_secs_f64();

        eprintln!(
            "Training data generation complete. Total positions: {}",
            total_positions
        );
        eprintln!(
            "Duration: {:.2}s ({:.2} positions/sec)",
            duration_secs,
            total_positions as f64 / duration_secs
        );

        Ok(total_positions)
    }
}

/// Determine the result of the game
fn determine_game_result(controller: &GameController) -> GameResult {
    let (move_count, _) = controller.game.get_moves();

    // No legal moves means either checkmate or stalemate
    if move_count == 0 {
        if controller.game.is_king_in_check(controller.game.side) {
            // Checkmate
            match controller.game.side {
                Color::White => GameResult::BlackWin,
                Color::Black => GameResult::WhiteWin,
            }
        } else {
            // Stalemate
            GameResult::Draw
        }
    } else {
        // Game is still ongoing, treat as draw for training purposes
        // (or could stop at a certain depth)
        GameResult::Draw
    }
}
