use criterion::{criterion_group, criterion_main, Criterion};
use othello::*;
use std::hint::black_box;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

fn bench_othello(c: &mut Criterion) {
    let mut board: Board = [[SquareState::Empty; BOARD_SIZE]; BOARD_SIZE];
    board[3][3] = SquareState::White;
    board[3][4] = SquareState::Black;
    board[4][3] = SquareState::Black;
    board[4][4] = SquareState::White;

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
            update_board(black_box(&mut test_board), 2, 3, SquareState::White);
        })
    });

    // Benchmark search performance for a fixed depth (better for performance comparison)
    let tt = TranspositionTable::new(1024); // Small TT for benchmark
    let abort = AtomicBool::new(false);
    c.bench_function("negamax_depth_6", |b| {
        b.iter(|| {
            negamax(
                black_box(&board),
                black_box(SquareState::White),
                black_box(6),
                black_box(i32::MIN + 1),
                black_box(i32::MAX),
                black_box(&abort),
                black_box(&tt),
            )
        })
    });

    // Overall search for 100ms
    let mut mid_game_board = board;
    update_board(&mut mid_game_board, 2, 3, SquareState::White);
    update_board(&mut mid_game_board, 2, 2, SquareState::Black);
    update_board(&mut mid_game_board, 4, 2, SquareState::White);

    c.bench_function("find_best_move_100ms", |b| {
        let moves = find_valid_moves(&mid_game_board, SquareState::Black);
        b.iter(|| {
            find_best_move(
                black_box(&mid_game_board),
                black_box(SquareState::Black),
                black_box(moves.clone()),
                black_box(Duration::from_millis(100)),
            )
        })
    });
}

criterion_group!(benches, bench_othello);
criterion_main!(benches);
