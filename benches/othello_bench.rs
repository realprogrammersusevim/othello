use criterion::{criterion_group, criterion_main, Criterion};
use othello::*;
use std::hint::black_box;
use std::time::Duration;

fn bench_othello(c: &mut Criterion) {
    let board = Board::new();

    c.bench_function("find_valid_moves", |b| {
        b.iter(|| find_valid_moves(black_box(&board), black_box(Player::WHITE)))
    });

    c.bench_function("evaluate", |b| {
        b.iter(|| evaluate(black_box(&board), black_box(Player::WHITE)))
    });

    c.bench_function("compute_hash", |b| {
        b.iter(|| compute_hash(black_box(&board), black_box(Player::WHITE)))
    });

    c.bench_function("make_move", |b| {
        b.iter(|| black_box(board).make_move(26, Player::WHITE))
    });

    // Use iter_batched with a fresh Engine per batch so TT starts cold each time,
    // matching the old benchmark's semantics (it also created a fresh TT per batch).
    c.bench_function("negamax_depth_6", |b| {
        b.iter_batched(
            || Engine::new(),
            |e| e.search_fixed_depth(black_box(&board), black_box(Player::WHITE), 6),
            criterion::BatchSize::LargeInput,
        )
    });

    // Overall search for 10ms on a mid-game position.
    // Engine lives outside the loop so the TT persists across iterations,
    // matching how the engine is used during real gameplay.
    let mid_game_board = board
        .make_move(26, Player::WHITE)
        .make_move(18, Player::BLACK)
        .make_move(20, Player::WHITE);

    let engine = Engine::new();
    c.bench_function("find_best_move_10ms", |b| {
        b.iter(|| {
            engine.find_best_move(
                black_box(&mid_game_board),
                black_box(Player::BLACK),
                black_box(Duration::from_millis(10)),
            )
        })
    });
}

criterion_group!(benches, bench_othello);
criterion_main!(benches);
