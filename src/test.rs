use rayon::iter::ParallelIterator;
use crate::GameController;
use std::collections::HashMap;
use std::fs;
use std::sync::Mutex;
use std::time::Instant;
use rayon::iter::IntoParallelRefIterator;

#[test]
fn test_zobrist_key_consistency() {
    let mut controller = GameController::new();

    let test_positions = [
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
        "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
        "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
    ];

    for position in test_positions {
        println!("Testing Zobrist consistency for: {}", position);
        controller.new_game_from_fen(position);

        let mut zobrist_position_map: HashMap<u64, String> = HashMap::new();
        let mut path = Vec::new();
        let mut failures = Vec::new();

        test_zobrist_consistency_recursive(
            &mut controller,
            4, // test depth
            &mut zobrist_position_map,
            &mut path,
            &mut failures,
        );

        if !failures.is_empty() {
            panic!(
                "Zobrist key consistency failures for position '{}':\n{}",
                position,
                failures.join("\n")
            );
        }
    }
}

fn test_zobrist_consistency_recursive(
    controller: &mut GameController,
    depth: usize,
    zobrist_position_map: &mut HashMap<u64, String>,
    path: &mut Vec<String>,
    failures: &mut Vec<String>,
) {
    if depth == 0 {
        return;
    }

    let current_zobrist = controller.game.zobrist_key;
    let current_fen = controller.game.get_fen();

    // Check if we've seen this Zobrist key before
    if let Some(previous_fen) = zobrist_position_map.get(&current_zobrist) {
        // Same Zobrist key should map to the same position
        if previous_fen != &current_fen {
            failures.push(format!(
                "Zobrist collision detected!\n  Key: 0x{:016x}\n  Path: {}\n  Current FEN: {}\n  Previous FEN: {}",
                current_zobrist,
                path.join(" -> "),
                current_fen,
                previous_fen
            ));
        }
    } else {
        // Record this Zobrist key and position
        zobrist_position_map.insert(current_zobrist, current_fen.clone());
    }

    // Test make/unmake move consistency
    let initial_zobrist = controller.game.zobrist_key;
    let initial_fen = controller.game.get_fen();

    let (moves, move_count) = controller.game.get_moves();

    for board_move in moves.into_iter().take(move_count) {
        let move_str = format!("{:?}", board_move); // Adjust this based on your move representation

        // Make the move
        controller.game.make_move(board_move.clone());
        path.push(move_str.clone());

        // Recursively test the new position
        test_zobrist_consistency_recursive(
            controller,
            depth - 1,
            zobrist_position_map,
            path,
            failures,
        );

        // Unmake the move
        controller.game.unmake_move();
        path.pop();

        // Verify that we're back to the original position
        let restored_zobrist = controller.game.zobrist_key;
        let restored_fen = controller.game.get_fen();

        if restored_zobrist != initial_zobrist {
            failures.push(format!(
                "Zobrist key not restored after unmake_move!\n  Path: {} -> {}\n  Initial: 0x{:016x}\n  After unmake: 0x{:016x}\n  Initial FEN: {}\n  Restored FEN: {}",
                path.join(" -> "),
                move_str,
                initial_zobrist,
                restored_zobrist,
                initial_fen,
                restored_fen
            ));
        }

        if restored_fen != initial_fen {
            failures.push(format!(
                "Position not restored after unmake_move!\n  Path: {} -> {}\n  Initial FEN: {}\n  Restored FEN: {}",
                path.join(" -> "),
                move_str,
                initial_fen,
                restored_fen
            ));
        }
    }
}

#[test]
fn test_zobrist_key_transposition_detection() {
    let mut controller = GameController::new();

    // Start from initial position
    controller.new_game();

    let mut seen_positions: HashMap<u64, (String, Vec<String>)> = HashMap::new();
    let mut transpositions_found = 0;

    test_transpositions_recursive(
        &mut controller,
        4, // depth
        &mut seen_positions,
        &mut Vec::new(),
        &mut transpositions_found,
    );

    println!(
        "Found {} transpositions in opening tree",
        transpositions_found
    );

    // We expect some transpositions in chess (e.g., Nf3 e6 Ng1 -> starting position)
    assert!(
        transpositions_found > 0,
        "Expected to find some transpositions in the opening"
    );
}

fn test_transpositions_recursive(
    controller: &mut GameController,
    depth: usize,
    seen_positions: &mut HashMap<u64, (String, Vec<String>)>,
    current_path: &mut Vec<String>,
    transpositions_found: &mut usize,
) {
    if depth == 0 {
        return;
    }

    let zobrist_key = controller.game.zobrist_key;
    let current_fen = controller.game.get_fen();
    let path_str = current_path.join(" ");

    if let Some((previous_fen, previous_path)) = seen_positions.get(&zobrist_key) {
        if previous_fen == &current_fen {
            *transpositions_found += 1;
            println!(
                "Transposition found!\n  Position: {}\n  Path 1: {}\n  Path 2: {}",
                current_fen,
                previous_path.join(" "),
                path_str
            );
        }
    } else {
        seen_positions.insert(zobrist_key, (current_fen, current_path.clone()));
    }

    let (moves, move_count) = controller.game.get_moves();

    for board_move in moves.into_iter().take(move_count) {
        let move_str = format!("{:?}", board_move);

        controller.game.make_move(board_move);
        current_path.push(move_str);

        test_transpositions_recursive(
            controller,
            depth - 1,
            seen_positions,
            current_path,
            transpositions_found,
        );

        current_path.pop();
        controller.game.unmake_move();
    }
}

#[test]
fn test_position() {
    let mut controller = GameController::new();

    for position in [
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
        "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
        "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
    ] {
        controller.new_game_from_fen(position);

        // Get the FEN back from the game
        let generated_fen = controller.game.get_fen();

        // Since the original FENs might be missing halfmove/fullmove counters,
        // we should compare just the parts that matter for position
        let original_parts: Vec<&str> = position.split_whitespace().collect();
        let generated_parts: Vec<&str> = generated_fen.split_whitespace().collect();

        // Compare piece placement
        assert_eq!(
            original_parts[0], generated_parts[0],
            "Piece placement mismatch for position: {}",
            position
        );

        // Compare active color
        assert_eq!(
            original_parts[1], generated_parts[1],
            "Active color mismatch for position: {}",
            position
        );

        // Compare castling rights
        assert_eq!(
            original_parts[2], generated_parts[2],
            "Castling rights mismatch for position: {}",
            position
        );

        // Compare en passant (if provided in original)
        if original_parts.len() > 3 {
            assert_eq!(
                original_parts[3], generated_parts[3],
                "En passant mismatch for position: {}",
                position
            );
        }

        // Compare halfmove clock (if provided in original)
        if original_parts.len() > 4 {
            assert_eq!(
                original_parts[4], generated_parts[4],
                "Halfmove clock mismatch for position: {}",
                position
            );
        }

        // Compare fullmove number (if provided in original)
        if original_parts.len() > 5 {
            assert_eq!(
                original_parts[5], generated_parts[5],
                "Fullmove number mismatch for position: {}",
                position
            );
        }
    }

    // Test with a complete standard starting position
    let starting_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    controller.new_game_from_fen(starting_fen);
    let generated = controller.game.get_fen();
    assert_eq!(starting_fen, generated, "Starting position FEN mismatch");
}

#[test]
fn test_perft_positions_easy() {
    test_perft_positions_from_file("data/small.txt", 1, 3);
}

#[test]
fn test_perft_positions_medium() {
    test_perft_positions_from_file("data/small.txt", 1, 4);
}

#[test]
fn test_perft_positions_hard() {
    test_perft_positions_from_file("data/large.txt", 1, 3);
}

#[test]
fn test_perft_positions_extreme() {
    test_perft_positions_from_file("data/large.txt", 1, 4);
}

fn test_perft_positions_from_file(file_path: &str, min_depth: usize, max_depth: usize) {
    // Read test positions from file
    let test_positions = load_perft_positions(file_path)
        .expect(&format!("Failed to load test positions from {}", file_path));

    // Create all test cases as a flat vector for parallel execution
    let test_cases: Vec<_> = test_positions
        .iter()
        .flat_map(|(fen, depth_counts)| {
            depth_counts
                .iter()
                .enumerate()
                .map(|(depth_idx, expected_count)| {
                    let depth = depth_idx + 1; // Convert from 0-based index to 1-based depth
                    (fen.clone(), depth, *expected_count)
                })
                .filter(|(_, depth, _)| min_depth <= *depth && *depth <= max_depth)
                .collect::<Vec<_>>()
        })
        .collect();

    println!("Running {} test cases in parallel...", test_cases.len());

    // Run all test cases in parallel and panic on first failure
    test_cases.par_iter().for_each(|(fen, depth, expected_count)| {
        let mut controller = GameController::new();
        controller.new_game_from_fen(fen);

        let start_time = Instant::now();
        let moves = controller.perft(*depth);
        let elapsed = start_time.elapsed();

        let total_nodes: usize = moves.iter().map(|(_, count)| count).sum();

        if total_nodes != *expected_count {
            panic!(
                "PERFT FAILURE: Position '{}' at depth {}: got {} nodes, expected {}",
                fen, depth, total_nodes, expected_count
            );
        }
    });

    println!("All {} test cases passed!", test_cases.len());
}

fn load_perft_positions(file_path: &str) -> Result<Vec<(String, Vec<usize>)>, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(file_path)?;
    let mut positions = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 2 {
            return Err(format!("Invalid line format: {}", line).into());
        }

        let fen = parts[0].to_string();
        let counts: Result<Vec<usize>, _> = parts[1..]
            .iter()
            .map(|s| s.trim().parse::<usize>())
            .collect();

        match counts {
            Ok(counts) => positions.push((fen, counts)),
            Err(e) => return Err(format!("Failed to parse counts in line '{}': {}", line, e).into()),
        }
    }

    Ok(positions)
}
