use crate::GameController;
use std::time::Instant;

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
        controller.new_game(Option::from(position));

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
    controller.new_game(Some(starting_fen));
    let generated = controller.game.get_fen();
    assert_eq!(starting_fen, generated, "Starting position FEN mismatch");
}

#[test]
fn test_perft_positions_easy() {
    test_perft_positions_depth(0, 3);
}

#[test]
fn test_perft_positions_hard() {
    test_perft_positions_depth(4, 5);
}

fn test_perft_positions_depth(min_depth: usize, max_depth: usize) {
    let mut controller = GameController::new();
    let mut failures: Vec<_> = Vec::new();
    let mut total = 0;

    // Yoinked from https://www.chessprogramming.org/Perft_Results
    let test_positions = [
        // Position 1: Starting position
        (
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            vec![(1, 20), (2, 400), (3, 8902), (4, 197281), (5, 4865609)],
        ),
        // Position 2: Kiwipete
        (
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq -",
            vec![(1, 48), (2, 2039), (3, 97862), (4, 4085603)],
        ),
        // Position 3: Position with en passant and castling
        ("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - -", vec![
            (1, 14),
            (2, 191),
            (3, 2812),
            (4, 43238),
        ]),
        // Position 4: Complex position with promotions
        (
            "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq -",
            vec![(1, 6), (2, 264), (3, 9467), (4, 422333)],
        ),
        // Position 5: Another complex position
        (
            "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
            vec![(1, 44), (2, 1486), (3, 62379), (4, 2103487)],
        ),
        // Position 6: Balanced middle game position
        (
            "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
            vec![(1, 46), (2, 2079), (3, 89890), (4, 3894594)],
        ),
    ];

    for (position_fen, depth_counts) in test_positions.iter() {
        println!("Testing position: {}", position_fen);
        controller.new_game(Some(position_fen));

        for &(depth, expected_count) in depth_counts {
            if !(min_depth <= depth && depth <= max_depth) {
                continue;
            }

            let start_time = Instant::now();
            let moves = controller.get_valid_moves(depth);
            let elapsed = start_time.elapsed();

            let total_nodes: usize = moves.iter().map(|(_, count)| count).sum();

            println!(
                "  Depth {}: {} nodes (expected: {}) - {:?}",
                depth, total_nodes, expected_count, elapsed
            );

            if total_nodes != expected_count {
                failures.push(format!(
                    "Position '{}' at depth {}: got {} nodes, expected {}",
                    position_fen, depth, total_nodes, expected_count
                ));
            }

            total += 1;
        }
        println!();
    }

    // Panic at the end with all failure information
    if !failures.is_empty() {
        let failure_summary = failures.join("\n  ");
        panic!(
            "Perft test failed with {}/{} error(s):\n  {}",
            failures.len(),
            total,
            failure_summary
        );
    }
}
