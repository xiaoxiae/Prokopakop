use crate::game::board::{BoardMove, BoardMoveExt, Game};
use crate::game::opening_book::GameResult;
use crate::utils::square::BoardSquareExt;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Debug)]
pub struct PgnGame {
    pub moves: Vec<BoardMove>,
    pub result: GameResult,
    pub average_elo: Option<u32>, // Added average ELO field
}

#[derive(Debug)]
pub enum PgnParseError {
    InvalidMove(String),
    InvalidResult(String),
    IoError(std::io::Error),
}

impl From<std::io::Error> for PgnParseError {
    fn from(error: std::io::Error) -> Self {
        PgnParseError::IoError(error)
    }
}

pub fn parse_pgn_file<P: AsRef<Path>>(path: P) -> Result<Vec<PgnGame>, PgnParseError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut games = Vec::new();
    let mut current_game_moves = String::new();
    let mut current_game_result = None;
    let mut white_elo: Option<u32> = None;
    let mut black_elo: Option<u32> = None;
    let mut in_header = false;

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();

        // Skip empty lines
        if line.is_empty() {
            continue;
        }

        // Header line (starts with '[')
        if line.starts_with('[') {
            in_header = true;

            // Parse result from header
            if line.starts_with("[Result ") {
                if let Some(result_str) = extract_header_value(line) {
                    current_game_result = Some(parse_result(&result_str)?);
                }
            }

            // Parse White ELO
            if line.starts_with("[WhiteElo ") {
                if let Some(elo_str) = extract_header_value(line) {
                    white_elo = elo_str.parse::<u32>().ok();
                }
            }

            // Parse Black ELO
            if line.starts_with("[BlackElo ") {
                if let Some(elo_str) = extract_header_value(line) {
                    black_elo = elo_str.parse::<u32>().ok();
                }
            }

            continue;
        }

        // If we were in header and now we're not, this is the start of moves
        if in_header && !line.starts_with('[') {
            in_header = false;
        }

        // Move line
        if !in_header {
            current_game_moves.push(' ');
            current_game_moves.push_str(line);

            // Check if this line contains the game result (ends with result pattern)
            if line.contains("1-0")
                || line.contains("0-1")
                || line.contains("1/2-1/2")
                || line.contains("*")
            {
                // Parse the game
                if let Some(result) = current_game_result {
                    let moves = parse_moves(&current_game_moves)?;

                    // Calculate average ELO if both ratings are available
                    let average_elo = match (white_elo, black_elo) {
                        (Some(w), Some(b)) => Some((w + b) / 2),
                        _ => None,
                    };

                    games.push(PgnGame {
                        moves,
                        result,
                        average_elo,
                    });
                }

                // Reset for next game
                current_game_moves.clear();
                current_game_result = None;
                white_elo = None;
                black_elo = None;
            }
        }
    }

    Ok(games)
}

fn extract_header_value(header_line: &str) -> Option<String> {
    // Extract value from [Tag "Value"] format
    let start = header_line.find('"')?;
    let end = header_line.rfind('"')?;
    if end > start {
        Some(header_line[start + 1..end].to_string())
    } else {
        None
    }
}

fn parse_result(result_str: &str) -> Result<GameResult, PgnParseError> {
    match result_str.trim() {
        "1-0" => Ok(GameResult::White),
        "0-1" => Ok(GameResult::Black),
        "1/2-1/2" => Ok(GameResult::Draw),
        "*" => Ok(GameResult::Draw), // Treat ongoing/unfinished games as draws
        _ => Err(PgnParseError::InvalidResult(result_str.to_string())),
    }
}

fn parse_moves(moves_text: &str) -> Result<Vec<BoardMove>, PgnParseError> {
    let mut moves = Vec::new();
    let mut game = Game::new(None); // Start from initial position

    // Clean up the moves text - remove move numbers, result, and comments
    let cleaned = clean_moves_text(moves_text);

    // Split into individual move tokens
    let tokens: Vec<&str> = cleaned
        .split_whitespace()
        .filter(|token| !token.is_empty() && !is_move_number(token))
        .collect();

    for token in tokens {
        // Skip result indicators
        if token == "1-0" || token == "0-1" || token == "1/2-1/2" || token == "*" {
            break;
        }

        // Parse algebraic notation to board move
        if let Some(board_move) = parse_algebraic_move(&mut game, token) {
            moves.push(board_move);
            game.make_move(board_move);
        } else {
            return Err(PgnParseError::InvalidMove(token.to_string()));
        }
    }

    Ok(moves)
}

fn clean_moves_text(text: &str) -> String {
    let mut result = String::new();
    let mut in_comment = false;
    let mut brace_depth = 0;

    for ch in text.chars() {
        match ch {
            '{' => {
                brace_depth += 1;
                in_comment = true;
            }
            '}' => {
                brace_depth -= 1;
                if brace_depth == 0 {
                    in_comment = false;
                }
            }
            ';' => {
                in_comment = true;
            }
            '\n' => {
                in_comment = false;
                result.push(' ');
            }
            _ => {
                if !in_comment {
                    result.push(ch);
                }
            }
        }
    }

    result
}

fn is_move_number(token: &str) -> bool {
    token.chars().next().map_or(false, |c| c.is_ascii_digit())
        && (token.ends_with('.') || token.ends_with("..."))
}

fn parse_algebraic_move(game: &mut Game, algebraic: &str) -> Option<BoardMove> {
    // Clean annotations from the move first
    let mut clean_move = algebraic.to_string();

    // Keep removing annotation characters from the end until none are left
    loop {
        let original_len = clean_move.len();
        clean_move = clean_move
            .trim_end_matches('+')
            .trim_end_matches('#')
            .trim_end_matches('!')
            .trim_end_matches('?')
            .to_string();

        // If no characters were removed, we're done
        if clean_move.len() == original_len {
            break;
        }
    }

    // Handle castling first
    if clean_move == "O-O" || clean_move == "0-0" {
        return if game.side == crate::game::pieces::Color::White {
            Some(BoardMove::regular(
                crate::utils::square::BoardSquare::E1,
                crate::utils::square::BoardSquare::G1,
            ))
        } else {
            Some(BoardMove::regular(
                crate::utils::square::BoardSquare::E8,
                crate::utils::square::BoardSquare::G8,
            ))
        };
    }

    if clean_move == "O-O-O" || clean_move == "0-0-0" {
        return if game.side == crate::game::pieces::Color::White {
            Some(BoardMove::regular(
                crate::utils::square::BoardSquare::E1,
                crate::utils::square::BoardSquare::C1,
            ))
        } else {
            Some(BoardMove::regular(
                crate::utils::square::BoardSquare::E8,
                crate::utils::square::BoardSquare::C8,
            ))
        };
    }

    // Get all legal moves for current position
    let (move_count, moves) = game.get_moves();

    // Try to match the algebraic notation to a legal move
    for i in 0..move_count {
        let board_move = moves[i];
        if matches_algebraic_notation(game, board_move, &clean_move) {
            return Some(board_move);
        }
    }

    None
}

fn matches_algebraic_notation(game: &Game, board_move: BoardMove, algebraic: &str) -> bool {
    use crate::game::pieces::Piece;
    use crate::utils::square::BoardSquareExt;

    // Remove check/checkmate indicators and annotations
    let mut clean_algebraic = algebraic.to_string();

    // Keep removing annotation characters from the end until none are left
    loop {
        let original_len = clean_algebraic.len();
        clean_algebraic = clean_algebraic
            .trim_end_matches('+')
            .trim_end_matches('#')
            .trim_end_matches('!')
            .trim_end_matches('?')
            .to_string();

        // If no characters were removed, we're done
        if clean_algebraic.len() == original_len {
            break;
        }
    }

    let from_square = board_move.get_from();
    let to_square = board_move.get_to();

    let piece_at_from = game.pieces[from_square as usize];

    if let Some((piece, _)) = piece_at_from {
        match piece {
            Piece::Pawn => {
                // Pawn moves
                if algebraic.len() == 2 {
                    // Simple pawn push like "e4"
                    return algebraic == to_square.unparse();
                } else if algebraic.len() == 4 && algebraic.contains('x') {
                    // Pawn capture like "exd5"
                    let parts: Vec<&str> = algebraic.split('x').collect();
                    if parts.len() == 2 {
                        let from_file = from_square.unparse().chars().next().unwrap();
                        return parts[0] == from_file.to_string()
                            && parts[1] == to_square.unparse();
                    }
                } else if clean_algebraic.contains('=') {
                    // Pawn promotion (e.g., "a1=Q", "axb8=N", "d8=Q")
                    let promotion_piece = board_move.get_promotion();
                    if let Some(promo) = promotion_piece {
                        let expected_promo_char = promo.to_char().to_ascii_uppercase();
                        let to_square_str = to_square.unparse();

                        // For simple promotion moves like "d8=Q"
                        if clean_algebraic == format!("{}={}", to_square_str, expected_promo_char) {
                            return true;
                        }

                        // For capture promotions like "axb8=N"
                        if clean_algebraic.contains('x') {
                            let from_file = from_square.unparse().chars().next().unwrap();
                            let expected_format =
                                format!("{}x{}={}", from_file, to_square_str, expected_promo_char);
                            if clean_algebraic == expected_format {
                                return true;
                            }
                        }
                    }
                }
            }
            other_piece => {
                // Piece moves like "Nf3", "Bxc4", "Qd1", "Raxb1"
                let piece_char = other_piece.to_char().to_ascii_uppercase();

                // Must start with piece character
                if !clean_algebraic.starts_with(piece_char) {
                    return false;
                }

                // Must end with destination square
                if !clean_algebraic.ends_with(&to_square.unparse()) {
                    return false;
                }

                // Check for capture indicator
                let has_capture = clean_algebraic.contains('x');
                let is_capture = game.pieces[to_square as usize].is_some();

                if has_capture != is_capture {
                    return false;
                }

                // For ambiguous moves, check if disambiguation matches
                if clean_algebraic.len() > 3 {
                    let middle_part = &clean_algebraic[1..clean_algebraic.len() - 2];
                    let middle_part = middle_part.replace('x', "");

                    if !middle_part.is_empty() {
                        let from_square_str = from_square.unparse();
                        // Check if disambiguation matches file or rank
                        if middle_part.len() == 1 {
                            let disam_char = middle_part.chars().next().unwrap();
                            let from_file = from_square_str.chars().next().unwrap();
                            let from_rank = from_square_str.chars().nth(1).unwrap();

                            if disam_char != from_file && disam_char != from_rank {
                                return false;
                            }
                        } else if middle_part.len() == 2 {
                            // Full square disambiguation
                            if middle_part != from_square_str {
                                return false;
                            }
                        }
                    }
                }

                return true;
            }
        }
    }

    false
}
