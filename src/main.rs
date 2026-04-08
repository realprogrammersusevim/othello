use rayon::prelude::*;
use std::io::{self, stdin, stdout, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use termion::event::{Event, Key};
use termion::input::TermRead;
use termion::raw::IntoRawMode;

const BOARD_SIZE: usize = 8;

#[derive(Copy, Clone, PartialEq)]
enum SquareState {
    Empty,
    White,
    Black,
}

type Board = [[SquareState; BOARD_SIZE]; BOARD_SIZE];

fn main() {
    print!("{}", termion::clear::All);
    let _raw = stdout().into_raw_mode().unwrap();

    let mut board: Board = [[SquareState::Empty; BOARD_SIZE]; BOARD_SIZE];

    // Set up initial state
    board[3][3] = SquareState::White;
    board[3][4] = SquareState::Black;
    board[4][3] = SquareState::Black;
    board[4][4] = SquareState::White;

    render_board(&board);
    let mut current_player = SquareState::White;
    let mut consecutive_passes = 0;
    for _ in 1..60 {
        let moves = find_valid_moves(&board, current_player);

        if moves.is_empty() {
            consecutive_passes += 1;
            if consecutive_passes >= 2 {
                break;
            }
            current_player = match current_player {
                SquareState::White => SquareState::Black,
                SquareState::Black => SquareState::White,
                SquareState::Empty => SquareState::Empty,
            };
            continue;
        }
        consecutive_passes = 0;

        let next_move: (usize, usize) = match current_player {
            SquareState::White => match get_user_move(&moves[..]) {
                Some(m) => m,
                None => return,
            },
            SquareState::Black => find_best_move(&board, current_player, moves.clone()),
            SquareState::Empty => (0, 0),
        };

        update_board(&mut board, next_move.0, next_move.1, current_player);
        render_board(&board);

        current_player = match current_player {
            SquareState::White => SquareState::Black,
            SquareState::Black => SquareState::White,
            SquareState::Empty => SquareState::Empty,
        }
    }

    let white = count_pieces(&board, SquareState::White);
    let black = count_pieces(&board, SquareState::Black);
    match white.cmp(&black) {
        std::cmp::Ordering::Greater => print!("White wins with {white} pieces!\r\n"),
        std::cmp::Ordering::Less => print!("Black wins with {black} pieces!\r\n"),
        std::cmp::Ordering::Equal => print!("Draw!\r\n"),
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn update_board(board: &mut Board, x: usize, y: usize, player: SquareState) {
    board[y][x] = player;
    let mut k: usize;
    let mut l: usize;
    let mut n: i8;
    let mut m: i8;

    for i in -1..=1 {
        for j in -1..=1 {
            if i == 0 && j == 0 {
                continue;
            }
            n = i;
            m = j;
            k = y;
            l = x;
            loop {
                k = (k as i8 + n) as usize;
                l = (l as i8 + m) as usize;
                if k > 7 || l > 7 {
                    break;
                }
                if board[k][l] == player {
                    while k != y || l != x {
                        k = (k as i8 - n) as usize;
                        l = (l as i8 - m) as usize;
                        board[k][l] = player;
                    }
                    break;
                }
                if board[k][l] == SquareState::Empty {
                    break;
                }
            }
        }
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn find_valid_moves(board: &Board, player: SquareState) -> Vec<(usize, usize)> {
    let mut moves: Vec<(usize, usize)> = Vec::new();
    let opponent = match player {
        SquareState::White => SquareState::Black,
        SquareState::Black => SquareState::White,
        SquareState::Empty => SquareState::Empty,
    };

    for i in 0..BOARD_SIZE {
        for j in 0..BOARD_SIZE {
            if board[i][j] != SquareState::Empty {
                continue;
            }
            for x in -1..=1 {
                for y in -1..=1 {
                    if x == 0 && y == 0 {
                        continue;
                    }
                    let mut n: i8 = i as i8 + x;
                    let mut m: i8 = j as i8 + y;
                    let mut found_opponent = false;

                    while n >= 0 && n < BOARD_SIZE as i8 && m >= 0 && m < BOARD_SIZE as i8 {
                        if board[n as usize][m as usize] == opponent {
                            found_opponent = true;
                        } else if board[n as usize][m as usize] == player {
                            if found_opponent {
                                moves.push((j, i));
                            }
                            break;
                        } else {
                            break;
                        }
                        n += x;
                        m += y;
                    }
                }
            }
        }
    }

    moves
}

fn count_pieces(board: &Board, player: SquareState) -> i32 {
    let mut player_count = 0;

    (0..BOARD_SIZE).for_each(|i| {
        for sq in &board[i] {
            if *sq == player {
                player_count += 1;
            }
        }
    });

    player_count
}

fn render_board(board: &Board) {
    print!("{}", termion::clear::All);
    print!("{}", termion::cursor::Goto(1, 1));
    print!("{}", termion::clear::CurrentLine);
    print!("  0 1 2 3 4 5 6 7\r\n");
    (0..BOARD_SIZE).for_each(|i| {
        print!("{}", termion::clear::CurrentLine);
        print!("{i} ");
        for sq in &board[i] {
            let symbol = match sq {
                SquareState::White => "\u{25cf} ", // black piece
                SquareState::Black => "\u{25cb} ", // white piece
                SquareState::Empty => "\u{00b7} ", // empty cell
            };
            print!("{symbol}");
        }
        print!("{i}\r\n");
    });
    print!("{}", termion::clear::CurrentLine);
    print!("  0 1 2 3 4 5 6 7\r\n");
    io::stdout().flush().unwrap();
}

// Safety cap on search depth — the time limit will nearly always trigger first
const MAX_DEPTH: usize = 60;

const TIME_LIMIT: Duration = Duration::from_secs(2);

// Endgame threshold: switch to exact piece-count when this many squares remain empty
const ENDGAME_EMPTY: i32 = 12;

#[rustfmt::skip]
const POSITION_WEIGHTS: [[i32; BOARD_SIZE]; BOARD_SIZE] = [
    [100, -20,  10,  5,  5,  10, -20, 100],
    [-20, -40,  -5, -5, -5,  -5, -40, -20],
    [ 10,  -5,  15,  3,  3,  15,  -5,  10],
    [  5,  -5,   3,  3,  3,   3,  -5,   5],
    [  5,  -5,   3,  3,  3,   3,  -5,   5],
    [ 10,  -5,  15,  3,  3,  15,  -5,  10],
    [-20, -40,  -5, -5, -5,  -5, -40, -20],
    [100, -20,  10,  5,  5,  10, -20, 100],
];

fn opposite(player: SquareState) -> SquareState {
    match player {
        SquareState::White => SquareState::Black,
        SquareState::Black => SquareState::White,
        SquareState::Empty => SquareState::Empty,
    }
}

fn positional_score(board: &Board, player: SquareState) -> i32 {
    let mut score = 0;
    for i in 0..BOARD_SIZE {
        for j in 0..BOARD_SIZE {
            if board[i][j] == player {
                score += POSITION_WEIGHTS[i][j];
            }
        }
    }
    score
}

/// Count discs that have at least one empty neighbour (exposed / unstable frontier).
fn frontier_discs(board: &Board, player: SquareState) -> i32 {
    let mut count = 0;
    'cell: for i in 0..BOARD_SIZE {
        for j in 0..BOARD_SIZE {
            if board[i][j] != player {
                continue;
            }
            for di in -1i32..=1 {
                for dj in -1i32..=1 {
                    if di == 0 && dj == 0 {
                        continue;
                    }
                    let ni = i as i32 + di;
                    let nj = j as i32 + dj;
                    if ni >= 0
                        && ni < BOARD_SIZE as i32
                        && nj >= 0
                        && nj < BOARD_SIZE as i32
                        && board[ni as usize][nj as usize] == SquareState::Empty
                    {
                        count += 1;
                        continue 'cell;
                    }
                }
            }
        }
    }
    count
}

fn evaluate(board: &Board, player: SquareState) -> i32 {
    let opp = opposite(player);
    let player_pieces = count_pieces(board, player);
    let opp_pieces = count_pieces(board, opp);
    let empty = (BOARD_SIZE * BOARD_SIZE) as i32 - player_pieces - opp_pieces;

    // Endgame: exact piece count dominates — scaled large to beat any heuristic value
    if empty <= ENDGAME_EMPTY {
        return (player_pieces - opp_pieces) * 500;
    }

    let pos = positional_score(board, player) - positional_score(board, opp);

    // Mobility: having more moves available is a strategic advantage
    let player_moves = find_valid_moves(board, player).len() as i32;
    let opp_moves = find_valid_moves(board, opp).len() as i32;
    let mobility = player_moves - opp_moves;

    // Frontier: fewer exposed discs = more stable position (negate the difference)
    let frontier = frontier_discs(board, player) - frontier_discs(board, opp);

    pos + mobility * 10 - frontier * 3
}

// Move ordering reuses the positional table: corners first, X-squares last
fn move_priority(mov: (usize, usize)) -> i32 {
    POSITION_WEIGHTS[mov.1][mov.0]
}

fn find_best_move(
    board: &Board,
    player: SquareState,
    mut moves: Vec<(usize, usize)>,
) -> (usize, usize) {
    moves.sort_by_key(|&m| -move_priority(m));

    let abort = Arc::new(AtomicBool::new(false));

    // Timer thread: fires the abort flag after TIME_LIMIT
    let abort_timer = Arc::clone(&abort);
    std::thread::spawn(move || {
        std::thread::sleep(TIME_LIMIT);
        abort_timer.store(true, Ordering::Relaxed);
    });

    let mut best_move = moves[0];
    // Tracks move order; re-sorted after each depth using scores from the previous
    // search (principal-variation ordering — improves alpha-beta cut rate at the next depth)
    let mut ordered = moves.clone();
    let start = Instant::now();

    for depth in 1..=MAX_DEPTH {
        if abort.load(Ordering::Relaxed) {
            break;
        }

        let abort_ref: &AtomicBool = &abort;
        let results: Vec<(usize, usize, i32)> = ordered
            .par_iter()
            .map(|&mov| {
                let mut b = board.to_owned();
                update_board(&mut b, mov.0, mov.1, player);
                let score = -negamax(
                    &b,
                    opposite(player),
                    depth - 1,
                    i32::MIN + 1,
                    i32::MAX,
                    abort_ref,
                );
                (mov.0, mov.1, score)
            })
            .collect();

        // Discard results from an interrupted depth — use last fully-completed result
        if abort.load(Ordering::Relaxed) {
            break;
        }

        if let Some(&(x, y, _)) = results.iter().max_by_key(|&&(_, _, s)| s) {
            best_move = (x, y);
        }

        // Re-order moves by descending score for the next depth
        ordered.sort_by_key(|&mov| {
            -results
                .iter()
                .find(|&&(x, y, _)| (x, y) == mov)
                .map_or(0, |&(_, _, s)| s)
        });

        // Early exit: if we've solved the position exactly, no deeper search needed
        if results.iter().any(|&(_, _, s)| s.abs() >= 5_000) {
            break;
        }

        let _ = start; // suppress unused warning; kept for future time-budget logging
    }

    best_move
}

fn negamax(
    board: &Board,
    player: SquareState,
    depth: usize,
    alpha: i32,
    beta: i32,
    abort: &AtomicBool,
) -> i32 {
    if abort.load(Ordering::Relaxed) {
        return 0; // value will be discarded by find_best_move
    }

    if depth == 0 {
        return evaluate(board, player);
    }

    let mut moves = find_valid_moves(board, player);
    if moves.is_empty() {
        let opp_moves = find_valid_moves(board, opposite(player));
        if opp_moves.is_empty() {
            // game over — exact result, scaled to dominate any heuristic score
            return (count_pieces(board, player) - count_pieces(board, opposite(player))) * 10_000;
        }
        // forced pass
        return -negamax(board, opposite(player), depth - 1, -beta, -alpha, abort);
    }

    moves.sort_by_key(|&m| -move_priority(m));

    let mut alpha = alpha;
    let mut best = i32::MIN + 1;
    for mov in moves {
        let mut b = board.to_owned();
        update_board(&mut b, mov.0, mov.1, player);
        let score = -negamax(&b, opposite(player), depth - 1, -beta, -alpha, abort);
        if score > best {
            best = score;
        }
        if score > alpha {
            alpha = score;
        }
        if alpha >= beta {
            break; // beta cut-off
        }
    }
    best
}

fn get_user_move(moves: &[(usize, usize)]) -> Option<(usize, usize)> {
    let labels = [
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'q', 'w', 'e', 'r', 't', 'y', 'u', 'i',
        'o', 'p', 'a', 's', 'd', 'f',
    ];

    for (i, mov) in moves.iter().enumerate().take(labels.len()) {
        print!("{}", termion::clear::CurrentLine);
        print!("{}: ({}, {})  \r\n", labels[i], mov.0, mov.1);
    }
    stdout().flush().unwrap();

    loop {
        for event in stdin().events() {
            let Ok(event) = event else { continue };
            if let Event::Key(Key::Char(c)) = event {
                if let Some(idx) = labels.iter().position(|&l| l == c) {
                    if idx < moves.len() {
                        return Some(moves[idx]);
                    }
                }
            }
            if let Event::Key(Key::Ctrl('c')) = event {
                return None;
            }
        }
    }
}
