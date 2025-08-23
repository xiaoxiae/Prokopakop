use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use prokopakop::controller::{ControllerMode, GameController};

fn perft_benchmark(c: &mut Criterion) {
    // Starting position perft benchmarks
    let mut group = c.benchmark_group("perft_starting_position");
    group
        .significance_level(0.1)
        .sample_size(5_000)
        .measurement_time(std::time::Duration::from_secs(20));

    // We want a high sample count, otherwise it's too noisy
    for depth in 1..=4 {
        group.bench_with_input(BenchmarkId::new("depth", depth), &depth, |b, &depth| {
            b.iter(|| {
                let mut controller = GameController::new();
                controller.initialize(ControllerMode::Play);
                controller.new_game();
                black_box(controller.perft(depth, true))
            });
        });
    }
    group.finish();

    // // Specific tactical positions
    // let test_positions = vec![
    //     (
    //         "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
    //         "starting_position",
    //     ),
    //     (
    //         "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
    //         "kiwipete_position",
    //     ),
    //     (
    //         "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
    //         "endgame_position",
    //     ),
    //     (
    //         "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
    //         "promotion_position",
    //     ),
    //     (
    //         "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
    //         "tactical_position",
    //     ),
    // ];

    // let mut group = c.benchmark_group("perft_various_positions");

    // for (fen, position_name) in test_positions {
    //     for depth in 1..=4 {
    //         group.bench_with_input(
    //             BenchmarkId::new(format!("{}_depth", position_name), depth),
    //             &(fen, depth),
    //             |b, &(fen, depth)| {
    //                 b.iter(|| {
    //                     let mut controller = GameController::new();
    //                     controller.initialize(ControllerMode::Play);
    //                     controller.new_game_from_fen(fen);
    //                     black_box(controller.perft(depth))
    //                 });
    //             },
    //         );
    //     }
    // }
    // group.finish();
}

criterion_group!(benches, perft_benchmark);
criterion_main!(benches);
