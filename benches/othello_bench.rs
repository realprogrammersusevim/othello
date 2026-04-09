use criterion::{criterion_group, criterion_main, Criterion};
use othello::*;
use std::hint::black_box;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

fn bench_othello(c: &mut Criterion) {
    let board = Board::new();

    c.bench_function("find_valid_moves", |b| {
        b.iter(|| find_valid_moves(black_box(&board), black_box(SquareState::White)))
    });

    c.bench_function("evaluate", |b| {
        b.iter(|| evaluate(black_box(&board), black_box(SquareState::White)))
    });

    c.bench_function("compute_hash", |b| {
        b.iter(|| compute_hash(black_box(&board), black_box(SquareState::White)))
    });

    c.bench_function("update_board", |b| {
        b.iter(|| {
            let mut test_board = board;
            update_board(black_box(&mut test_board), 26, SquareState::White);
        })
    });

    // Benchmark search performance for a fixed depth (better for performance comparison)
    let abort = AtomicBool::new(false);
    c.bench_function("negamax_depth_6", |b| {
        b.iter_batched(
            || TranspositionTable::new(1 << 20),
            |tt| {
                negamax(
                    black_box(&board),
                    black_box(SquareState::White),
                    6,
                    i32::MIN + 1,
                    i32::MAX,
                    &abort,
                    &tt,
                )
            },
            criterion::BatchSize::LargeInput,
        )
    });

    // Overall search for 100ms
    let mut mid_game_board = board;
    update_board(&mut mid_game_board, 26, SquareState::White);
    update_board(&mut mid_game_board, 18, SquareState::Black);
    update_board(&mut mid_game_board, 20, SquareState::White);

    c.bench_function("find_best_move_10ms", |b| {
        let moves = find_valid_moves(&mid_game_board, SquareState::Black);
        b.iter(|| {
            find_best_move(
                black_box(&mid_game_board),
                black_box(SquareState::Black),
                black_box(moves.clone()),
                black_box(Duration::from_millis(10)),
            )
        })
    });
}

criterion_group!(benches, bench_othello);
criterion_main!(benches);
