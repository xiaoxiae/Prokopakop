use crate::game::board::{BoardMove, BoardMoveExt};
use crate::game::pieces::Color;

/// Search limits and parameters
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct SearchLimits {
    pub max_depth: Option<usize>,
    pub max_nodes: Option<u64>,
    pub max_time_ms: Option<u64>,
    pub moves: Vec<BoardMove>, // TODO: implement this!
    pub infinite: bool,
    pub exact: bool, // Whether to actually search for this amount (even for forced moves)
}

/// Search parameters from UCI go command
#[derive(Debug, Clone)]
pub struct SearchParams {
    pub depth: Option<usize>,        // search to depth x
    pub movetime: Option<u64>,       // search exactly x milliseconds
    pub wtime: Option<u64>,          // white has x milliseconds left on clock
    pub btime: Option<u64>,          // black has x milliseconds left on clock
    pub winc: Option<u64>,           // white increment per move in milliseconds
    pub binc: Option<u64>,           // black increment per move in milliseconds
    pub movestogo: Option<usize>,    // there are x moves to the next time control
    pub nodes: Option<u64>,          // search x nodes only
    pub infinite: bool,              // search until "stop" command
    pub ponder: bool,                // search in ponder mode
    pub searchmoves: Vec<BoardMove>, // restrict search to these moves only
}

impl Default for SearchParams {
    fn default() -> Self {
        Self {
            depth: None,
            movetime: None,
            wtime: None,
            btime: None,
            winc: None,
            binc: None,
            movestogo: None,
            nodes: None,
            infinite: false,
            ponder: false,
            searchmoves: Vec::new(),
        }
    }
}

impl SearchParams {
    pub fn parse(params: Vec<String>) -> Self {
        let mut search_params = SearchParams::default();
        let mut iter = params.iter();

        while let Some(param) = iter.next() {
            match param.as_str() {
                "depth" => {
                    if let Some(value) = iter.next() {
                        search_params.depth = value.parse().ok();
                    }
                }
                "movetime" => {
                    if let Some(value) = iter.next() {
                        search_params.movetime = value.parse().ok();
                    }
                }
                "wtime" => {
                    if let Some(value) = iter.next() {
                        search_params.wtime = value.parse().ok();
                    }
                }
                "btime" => {
                    if let Some(value) = iter.next() {
                        search_params.btime = value.parse().ok();
                    }
                }
                "winc" => {
                    if let Some(value) = iter.next() {
                        search_params.winc = value.parse().ok();
                    }
                }
                "binc" => {
                    if let Some(value) = iter.next() {
                        search_params.binc = value.parse().ok();
                    }
                }
                "movestogo" => {
                    if let Some(value) = iter.next() {
                        search_params.movestogo = value.parse().ok();
                    }
                }
                "nodes" => {
                    if let Some(value) = iter.next() {
                        search_params.nodes = value.parse().ok();
                    }
                }
                "infinite" => {
                    search_params.infinite = true;
                }
                "ponder" => {
                    search_params.ponder = true;
                }
                "searchmoves" => {
                    // Collect all remaining moves
                    while let Some(move_str) = iter.next() {
                        // Check if this is another parameter (not a move)
                        if [
                            "depth",
                            "movetime",
                            "wtime",
                            "btime",
                            "winc",
                            "binc",
                            "movestogo",
                            "nodes",
                            "infinite",
                            "ponder",
                        ]
                        .contains(&move_str.as_str())
                        {
                            // Put it back by breaking and letting the outer loop handle it
                            // Note: This is a simplified approach. In production, you might want
                            // to handle this differently
                            break;
                        }

                        if let Some(board_move) = BoardMove::parse(move_str) {
                            search_params.searchmoves.push(board_move);
                        }
                    }
                }
                _ => {
                    // Unknown parameter, skip
                    eprintln!("Unknown go parameter: {}", param);
                }
            }
        }

        search_params
    }

    pub fn calculate_move_time(&self, color: Color, move_overhead: u64) -> Option<u64> {
        // If movetime is specified, use that (subtract move overhead)
        if let Some(movetime) = self.movetime {
            return Some(movetime.saturating_sub(move_overhead));
        }

        // If infinite search, no time limit
        if self.infinite {
            return None;
        }

        // Get the time and increment for the current side
        let (time_left, increment) = match color {
            Color::White => (self.wtime, self.winc.unwrap_or(0)),
            Color::Black => (self.btime, self.binc.unwrap_or(0)),
        };

        // If we have time control information
        if let Some(time) = time_left {
            let moves_remaining = self.movestogo.unwrap_or(30) as u64; // Assume 30 moves if not specified

            // Apply move overhead - this accounts for network/GUI delays
            let available_time = time.saturating_sub(move_overhead);

            // Spend most of increment
            let base_time = available_time / moves_remaining.max(1);
            let allocated_time = base_time + (increment * 8 / 10);

            // Min 10ms for move (but ensure we don't go below 1ms due to overhead)
            Some(
                allocated_time
                    .max(10)
                    .saturating_sub(move_overhead.min(allocated_time / 2)),
            )
        } else {
            None
        }
    }
}
