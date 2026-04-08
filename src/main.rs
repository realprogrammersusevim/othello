use rayon::prelude::*;
use std::cell::UnsafeCell;
use std::io::{self, stdin, stdout, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;
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

        let (next_move, ai_depth) = match current_player {
            SquareState::White => (
                match get_user_move(&moves[..]) {
                    Some(m) => m,
                    None => return,
                },
                0,
            ),
            SquareState::Black => find_best_move(&board, current_player, moves.clone()),
            SquareState::Empty => ((0, 0), 0),
        };

        update_board(&mut board, next_move.0, next_move.1, current_player);
        render_board(&board);
        if current_player == SquareState::Black && ai_depth > 0 {
            print!("AI searched to depth {ai_depth}\r\n");
            io::stdout().flush().unwrap();
        }

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
            'cell: for x in -1..=1 {
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
                                break 'cell;
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

const TIME_LIMIT: Duration = Duration::from_secs(10);

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

// ---------- Zobrist hashing ----------

struct ZobristTable {
    pieces: [[[u64; 3]; BOARD_SIZE]; BOARD_SIZE],
    black_to_move: u64,
}

static ZOBRIST: OnceLock<ZobristTable> = OnceLock::new();

fn init_zobrist() -> ZobristTable {
    let mut s: u64 = 0xcafef00d_deadbeef;
    let next = |s: &mut u64| -> u64 {
        *s = s.wrapping_add(0x9e3779b97f4a7c15);
        let mut x = *s;
        x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
        x ^ (x >> 31)
    };
    let mut pieces = [[[0u64; 3]; BOARD_SIZE]; BOARD_SIZE];
    for row in pieces.iter_mut() {
        for cell in row.iter_mut() {
            for slot in cell.iter_mut() {
                *slot = next(&mut s);
            }
        }
    }
    ZobristTable {
        pieces,
        black_to_move: next(&mut s),
    }
}

fn compute_hash(board: &Board, player: SquareState) -> u64 {
    let zt = ZOBRIST.get_or_init(init_zobrist);
    let mut hash: u64 = 0;
    for i in 0..BOARD_SIZE {
        for j in 0..BOARD_SIZE {
            let k = match board[i][j] {
                SquareState::Empty => 0,
                SquareState::White => 1,
                SquareState::Black => 2,
            };
            hash ^= zt.pieces[i][j][k];
        }
    }
    if player == SquareState::Black {
        hash ^= zt.black_to_move;
    }
    hash
}

// ---------- Transposition table ----------

const TT_SIZE: usize = 1 << 20; // ~1M entries, ~16 MB

const TT_NONE: u8 = 0;
const TT_EXACT: u8 = 1;
const TT_LOWER: u8 = 2; // fail-high: score >= beta
const TT_UPPER: u8 = 3; // fail-low:  score <= alpha

#[derive(Clone, Copy)]
#[repr(C)]
struct TTEntry {
    key: u64,
    score: i32,
    flag: u8,
    depth: u8,
    best_x: u8, // 255 = no move stored
    best_y: u8,
}

impl Default for TTEntry {
    fn default() -> Self {
        TTEntry {
            key: 0,
            score: 0,
            flag: TT_NONE,
            depth: 0,
            best_x: 255,
            best_y: 255,
        }
    }
}

struct TranspositionTable {
    data: Vec<UnsafeCell<TTEntry>>,
    mask: usize,
}

// Lockless TT: races cause benign torn reads/writes caught by the key check.
unsafe impl Sync for TranspositionTable {}
unsafe impl Send for TranspositionTable {}

impl TranspositionTable {
    fn new(size: usize) -> Self {
        assert!(size.is_power_of_two());
        TranspositionTable {
            data: (0..size)
                .map(|_| UnsafeCell::new(TTEntry::default()))
                .collect(),
            mask: size - 1,
        }
    }

    fn probe(&self, hash: u64) -> Option<TTEntry> {
        let entry = unsafe { *self.data[hash as usize & self.mask].get() };
        if entry.flag != TT_NONE && entry.key == hash {
            Some(entry)
        } else {
            None
        }
    }

    fn store(&self, hash: u64, depth: u8, score: i32, flag: u8, best: Option<(usize, usize)>) {
        let slot = unsafe { &mut *self.data[hash as usize & self.mask].get() };
        let (bx, by) = best.map_or((255u8, 255u8), |(x, y)| (x as u8, y as u8));
        *slot = TTEntry {
            key: hash,
            score,
            flag,
            depth,
            best_x: bx,
            best_y: by,
        };
    }
}

static TT: OnceLock<TranspositionTable> = OnceLock::new();

// ---------- Evaluation ----------

fn evaluate(board: &Board, player: SquareState) -> i32 {
    let opp = opposite(player);
    let mut player_pieces = 0i32;
    let mut opp_pieces = 0i32;
    let mut player_pos = 0i32;
    let mut opp_pos = 0i32;
    let mut player_frontier = 0i32;
    let mut opp_frontier = 0i32;

    // Single pass: piece counts, positional scores, and frontier discs
    for i in 0..BOARD_SIZE {
        for j in 0..BOARD_SIZE {
            let sq = board[i][j];
            if sq == SquareState::Empty {
                continue;
            }
            let is_player = sq == player;
            if is_player {
                player_pieces += 1;
                player_pos += POSITION_WEIGHTS[i][j];
            } else {
                opp_pieces += 1;
                opp_pos += POSITION_WEIGHTS[i][j];
            }
            'nb: for di in -1i32..=1 {
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
                        if is_player {
                            player_frontier += 1;
                        } else {
                            opp_frontier += 1;
                        }
                        break 'nb;
                    }
                }
            }
        }
    }

    let empty = (BOARD_SIZE * BOARD_SIZE) as i32 - player_pieces - opp_pieces;

    // Endgame: exact piece count dominates — scaled large to beat any heuristic value
    if empty <= ENDGAME_EMPTY {
        return (player_pieces - opp_pieces) * 500;
    }

    let pos = player_pos - opp_pos;

    // Mobility: having more moves available is a strategic advantage
    let player_moves = find_valid_moves(board, player).len() as i32;
    let opp_moves = find_valid_moves(board, opp).len() as i32;
    let mobility = player_moves - opp_moves;

    // Frontier: fewer exposed discs = more stable position
    let frontier = player_frontier - opp_frontier;

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
) -> ((usize, usize), usize) {
    moves.sort_by_key(|&m| -move_priority(m));

    let abort = Arc::new(AtomicBool::new(false));

    // Timer thread: fires the abort flag after TIME_LIMIT
    let abort_timer = Arc::clone(&abort);
    std::thread::spawn(move || {
        std::thread::sleep(TIME_LIMIT);
        abort_timer.store(true, Ordering::Relaxed);
    });

    let mut best_move = moves[0];
    let mut completed_depth = 0usize;
    // Tracks move order; re-sorted after each depth using scores from the previous
    // search (principal-variation ordering — improves alpha-beta cut rate at the next depth)
    let mut ordered = moves.clone();
    let tt = TT.get_or_init(|| TranspositionTable::new(TT_SIZE));

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
                    tt,
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
        completed_depth = depth;

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
    }

    (best_move, completed_depth)
}

fn negamax(
    board: &Board,
    player: SquareState,
    depth: usize,
    alpha: i32,
    beta: i32,
    abort: &AtomicBool,
    tt: &TranspositionTable,
) -> i32 {
    if abort.load(Ordering::Relaxed) {
        return 0; // value will be discarded by find_best_move
    }

    let hash = compute_hash(board, player);
    let orig_alpha = alpha;
    let mut alpha = alpha;
    let mut tt_best: Option<(usize, usize)> = None;

    if let Some(entry) = tt.probe(hash) {
        if entry.best_x < BOARD_SIZE as u8 {
            tt_best = Some((entry.best_x as usize, entry.best_y as usize));
        }
        if entry.depth >= depth as u8 {
            let s = entry.score;
            match entry.flag {
                TT_EXACT => return s,
                TT_LOWER => {
                    if s >= beta {
                        return s;
                    }
                    alpha = alpha.max(s);
                }
                TT_UPPER => {
                    if s <= alpha {
                        return s;
                    }
                }
                _ => {}
            }
        }
    }

    if depth == 0 {
        let score = evaluate(board, player);
        tt.store(hash, 0, score, TT_EXACT, None);
        return score;
    }

    let mut moves = find_valid_moves(board, player);
    if moves.is_empty() {
        let opp_moves = find_valid_moves(board, opposite(player));
        if opp_moves.is_empty() {
            // game over — exact result, scaled to dominate any heuristic score
            let score =
                (count_pieces(board, player) - count_pieces(board, opposite(player))) * 10_000;
            tt.store(hash, 255, score, TT_EXACT, None);
            return score;
        }
        // forced pass — position didn't advance, so don't decrement depth
        return -negamax(board, opposite(player), depth, -beta, -alpha, abort, tt);
    }

    // TT hash move first, then positional weights
    moves.sort_by_key(|&m| {
        if Some(m) == tt_best {
            i32::MIN
        } else {
            -move_priority(m)
        }
    });

    let mut best = i32::MIN + 1;
    let mut best_move = moves[0];
    for &mov in &moves {
        let mut b = board.to_owned();
        update_board(&mut b, mov.0, mov.1, player);
        let score = -negamax(&b, opposite(player), depth - 1, -beta, -alpha, abort, tt);
        if abort.load(Ordering::Relaxed) {
            return 0; // don't store aborted results
        }
        if score > best {
            best = score;
            best_move = mov;
        }
        if score > alpha {
            alpha = score;
        }
        if alpha >= beta {
            break; // beta cut-off
        }
    }

    let flag = if best >= beta {
        TT_LOWER
    } else if best > orig_alpha {
        TT_EXACT
    } else {
        TT_UPPER
    };
    tt.store(hash, depth as u8, best, flag, Some(best_move));

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
