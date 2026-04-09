use rayon::prelude::*;
use std::cell::UnsafeCell;
use std::io::{self, stdin, stdout, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;
use termion::event::{Event, Key};
use termion::input::TermRead;

// ---------------------------------------------------------------------------
// Board
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, PartialEq, Debug, Default)]
pub struct Board {
    pub black: u64,
    pub white: u64,
}

impl Board {
    pub fn new() -> Self {
        let mut board = Board::default();
        board.white |= (1 << 27) | (1 << 36);
        board.black |= (1 << 28) | (1 << 35);
        board
    }

    pub fn get(&self, x: usize, y: usize) -> SquareState {
        let bit = 1 << (y * 8 + x);
        if (self.black & bit) != 0 {
            SquareState::Black
        } else if (self.white & bit) != 0 {
            SquareState::White
        } else {
            SquareState::Empty
        }
    }

    pub fn set(&mut self, x: usize, y: usize, state: SquareState) {
        let bit = 1 << (y * 8 + x);
        self.black &= !bit;
        self.white &= !bit;
        match state {
            SquareState::Black => self.black |= bit,
            SquareState::White => self.white |= bit,
            SquareState::Empty => {}
        }
    }

    /// Returns `(player_bits, opponent_bits)`.
    #[inline]
    pub fn player_opp(&self, p: Player) -> (u64, u64) {
        if p == Player::BLACK {
            (self.black, self.white)
        } else {
            (self.white, self.black)
        }
    }

    /// All legal destination squares for `player` as a bitmask iterator.
    #[inline]
    pub fn valid_moves(&self, p: Player) -> MoveMask {
        let (pl, op) = self.player_opp(p);
        MoveMask(get_moves(pl, op))
    }

    /// Apply a move and return the resulting board (consuming `self`).
    #[inline]
    pub fn make_move(mut self, bit_idx: u8, player: Player) -> Self {
        let move_bit = 1u64 << bit_idx;
        let (p, o) = if player == Player::BLACK {
            (&mut self.black, &mut self.white)
        } else {
            (&mut self.white, &mut self.black)
        };
        let flips = get_flips(*p, *o, move_bit);
        *p |= move_bit | flips;
        *o &= !flips;
        self
    }
}

// ---------------------------------------------------------------------------
// Player — a two-valued type; cannot accidentally be "Empty"
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Player(bool);

impl Player {
    pub const BLACK: Self = Player(true);
    pub const WHITE: Self = Player(false);

    #[inline]
    pub fn opposite(self) -> Self {
        Player(!self.0)
    }

    #[inline]
    pub fn to_square_state(self) -> SquareState {
        if self.0 {
            SquareState::Black
        } else {
            SquareState::White
        }
    }
}

// ---------------------------------------------------------------------------
// SquareState — used only for board display / piece counting
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum SquareState {
    Empty,
    White,
    Black,
}

// ---------------------------------------------------------------------------
// MoveMask — zero-alloc bit iterator over legal moves
// ---------------------------------------------------------------------------

#[derive(Copy, Clone)]
pub struct MoveMask(pub u64);

impl MoveMask {
    #[inline]
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl Iterator for MoveMask {
    type Item = u8;

    #[inline]
    fn next(&mut self) -> Option<u8> {
        if self.0 == 0 {
            return None;
        }
        let bit = self.0.trailing_zeros() as u8;
        self.0 &= self.0 - 1;
        Some(bit)
    }
}

// ---------------------------------------------------------------------------
// Bitboard primitives
// ---------------------------------------------------------------------------

const A_FILE: u64 = 0x0101010101010101;
const H_FILE: u64 = 0x8080808080808080;

const DIRS: [(i8, u64); 8] = [
    (1, !A_FILE),   // Right
    (-1, !H_FILE),  // Left
    (8, !0u64),     // Down
    (-8, !0u64),    // Up
    (7, !H_FILE),   // DownLeft
    (-7, !A_FILE),  // UpRight
    (9, !A_FILE),   // DownRight
    (-9, !H_FILE),  // UpLeft
];

fn get_moves(player: u64, opponent: u64) -> u64 {
    let empty = !(player | opponent);
    let mut moves = 0u64;
    for (shift, mask) in DIRS {
        let mut candidates = shift_mask(player, shift, mask) & opponent;
        for _ in 0..5 {
            candidates |= shift_mask(candidates, shift, mask) & opponent;
        }
        moves |= shift_mask(candidates, shift, mask) & empty;
    }
    moves
}

#[inline(always)]
fn shift_mask(b: u64, shift: i8, mask: u64) -> u64 {
    if shift > 0 {
        (b << shift) & mask
    } else {
        (b >> -shift) & mask
    }
}

fn get_flips(player: u64, opponent: u64, move_bit: u64) -> u64 {
    let mut flips = 0u64;
    for (shift, mask) in DIRS {
        let candidates = shift_mask(move_bit, shift, mask) & opponent;
        if candidates == 0 {
            continue;
        }
        let mut current_ray = candidates;
        for _ in 0..6 {
            let next = shift_mask(current_ray, shift, mask);
            if (next & opponent) != 0 {
                current_ray |= next;
            } else {
                if (next & player) != 0 {
                    flips |= current_ray;
                }
                break;
            }
        }
    }
    flips
}

// ---------------------------------------------------------------------------
// Public helpers
// ---------------------------------------------------------------------------

pub const BOARD_SIZE: usize = 8;

pub fn find_valid_moves(board: &Board, player: Player) -> Vec<(usize, usize)> {
    board
        .valid_moves(player)
        .map(|i| (i as usize % 8, i as usize / 8))
        .collect()
}

pub fn count_pieces(board: &Board, player: SquareState) -> i32 {
    match player {
        SquareState::Black => board.black.count_ones() as i32,
        SquareState::White => board.white.count_ones() as i32,
        SquareState::Empty => (64 - (board.black | board.white).count_ones()) as i32,
    }
}

pub fn render_board(board: &Board) {
    print!("{}", termion::clear::All);
    print!("{}", termion::cursor::Goto(1, 1));
    print!("{}", termion::clear::CurrentLine);
    print!("  0 1 2 3 4 5 6 7\r\n");
    for i in 0..BOARD_SIZE {
        print!("{}", termion::clear::CurrentLine);
        print!("{i} ");
        for j in 0..BOARD_SIZE {
            let symbol = match board.get(j, i) {
                SquareState::White => "\u{25cf} ",
                SquareState::Black => "\u{25cb} ",
                SquareState::Empty => "\u{00b7} ",
            };
            print!("{symbol}");
        }
        print!("{i}\r\n");
    }
    print!("{}", termion::clear::CurrentLine);
    print!("  0 1 2 3 4 5 6 7\r\n");
    io::stdout().flush().unwrap();
}

// ---------------------------------------------------------------------------
// Evaluation
// ---------------------------------------------------------------------------

const ENDGAME_EMPTY: i32 = 12;

#[rustfmt::skip]
const POSITION_WEIGHTS: [i32; 64] = [
    100, -20,  10,  5,  5,  10, -20, 100,
    -20, -40,  -5, -5, -5,  -5, -40, -20,
     10,  -5,  15,  3,  3,  15,  -5,  10,
      5,  -5,   3,  3,  3,   3,  -5,   5,
      5,  -5,   3,  3,  3,   3,  -5,   5,
     10,  -5,  15,  3,  3,  15,  -5,  10,
    -20, -40,  -5, -5, -5,  -5, -40, -20,
    100, -20,  10,  5,  5,  10, -20, 100,
];

pub fn evaluate(board: &Board, player: Player) -> i32 {
    let (p_board, o_board) = board.player_opp(player);

    let p_count = p_board.count_ones() as i32;
    let o_count = o_board.count_ones() as i32;
    let empty = 64 - p_count - o_count;

    if empty <= ENDGAME_EMPTY {
        return (p_count - o_count) * 500;
    }

    let mut p_pos = 0i32;
    let mut o_pos = 0i32;
    let mut b = p_board;
    while b != 0 {
        let i = b.trailing_zeros() as usize;
        p_pos += POSITION_WEIGHTS[i];
        b &= b - 1;
    }
    let mut b = o_board;
    while b != 0 {
        let i = b.trailing_zeros() as usize;
        o_pos += POSITION_WEIGHTS[i];
        b &= b - 1;
    }

    let p_moves = get_moves(p_board, o_board).count_ones() as i32;
    let o_moves = get_moves(o_board, p_board).count_ones() as i32;
    let mobility = p_moves - o_moves;

    let empty_mask = !(p_board | o_board);
    let p_frontier = count_frontier(p_board, empty_mask);
    let o_frontier = count_frontier(o_board, empty_mask);
    let frontier = p_frontier - o_frontier;

    (p_pos - o_pos) + mobility * 10 - frontier * 3
}

fn count_frontier(board: u64, empty: u64) -> i32 {
    let mut frontier = 0u64;
    for (shift, mask) in DIRS {
        frontier |= shift_mask(board, shift, mask) & empty;
    }
    frontier.count_ones() as i32
}

#[inline]
fn move_priority(bit_idx: u8) -> i32 {
    POSITION_WEIGHTS[bit_idx as usize]
}

// ---------------------------------------------------------------------------
// Zobrist hashing
// ---------------------------------------------------------------------------

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

pub fn compute_hash(board: &Board, player: Player) -> u64 {
    let zt = ZOBRIST.get_or_init(init_zobrist);
    let mut hash: u64 = 0;
    let mut b = board.black;
    while b != 0 {
        let i = b.trailing_zeros() as usize;
        hash ^= zt.pieces[i / 8][i % 8][2];
        b &= b - 1;
    }
    let mut w = board.white;
    while w != 0 {
        let i = w.trailing_zeros() as usize;
        hash ^= zt.pieces[i / 8][i % 8][1];
        w &= w - 1;
    }
    if player == Player::BLACK {
        hash ^= zt.black_to_move;
    }
    hash
}

// ---------------------------------------------------------------------------
// Transposition table
// ---------------------------------------------------------------------------

const TT_SIZE: usize = 1 << 20;
const TT_NONE: u8 = 0;
const TT_EXACT: u8 = 1;
const TT_LOWER: u8 = 2;
const TT_UPPER: u8 = 3;

#[derive(Clone, Copy)]
#[repr(C)]
struct TTEntry {
    key: u64,
    score: i32,
    flag: u8,
    depth: u8,
    best_move: u8, // 255 = none
}

impl Default for TTEntry {
    fn default() -> Self {
        TTEntry {
            key: 0,
            score: 0,
            flag: TT_NONE,
            depth: 0,
            best_move: 255,
        }
    }
}

struct TranspositionTable {
    data: Vec<UnsafeCell<TTEntry>>,
    mask: usize,
}

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

    fn store(&self, hash: u64, depth: u8, score: i32, flag: u8, best_move: Option<u8>) {
        let slot = unsafe { &mut *self.data[hash as usize & self.mask].get() };
        *slot = TTEntry {
            key: hash,
            score,
            flag,
            depth,
            best_move: best_move.unwrap_or(255),
        };
    }
}

// ---------------------------------------------------------------------------
// Engine — owns the transposition table; drives iterative-deepening search
// ---------------------------------------------------------------------------

const MAX_DEPTH: usize = 60;

pub struct Engine {
    tt: TranspositionTable,
}

impl Default for Engine {
    fn default() -> Self {
        Engine::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        Engine {
            tt: TranspositionTable::new(TT_SIZE),
        }
    }

    /// Run negamax at a fixed depth with no time limit (useful for benchmarking).
    pub fn search_fixed_depth(&self, board: &Board, player: Player, depth: usize) -> i32 {
        let abort = AtomicBool::new(false);
        self.negamax(board, player, depth, i32::MIN + 1, i32::MAX, &abort)
    }

    /// Returns the best move as `(col, row)` and the completed search depth.
    pub fn find_best_move(
        &self,
        board: &Board,
        player: Player,
        time_limit: Duration,
    ) -> ((usize, usize), usize) {
        let moves_mask = board.valid_moves(player);
        if moves_mask.is_empty() {
            return ((0, 0), 0);
        }

        let abort = Arc::new(AtomicBool::new(false));
        let abort_timer = Arc::clone(&abort);
        std::thread::spawn(move || {
            std::thread::sleep(time_limit);
            abort_timer.store(true, Ordering::Relaxed);
        });

        let mut ordered: Vec<u8> = moves_mask.collect();
        ordered.sort_by_key(|&m| -move_priority(m));

        let mut best_move_idx = ordered[0];
        let mut completed_depth = 0usize;

        for depth in 1..=MAX_DEPTH {
            if abort.load(Ordering::Relaxed) {
                break;
            }
            let abort_ref: &AtomicBool = &abort;
            let results: Vec<(u8, i32)> = ordered
                .par_iter()
                .map(|&m| {
                    let b = board.make_move(m, player);
                    let score = -self.negamax(
                        &b,
                        player.opposite(),
                        depth - 1,
                        i32::MIN + 1,
                        i32::MAX,
                        abort_ref,
                    );
                    (m, score)
                })
                .collect();

            if abort.load(Ordering::Relaxed) {
                break;
            }
            if let Some(&(m, _)) = results.iter().max_by_key(|&&(_, s)| s) {
                best_move_idx = m;
            }
            completed_depth = depth;
            ordered.sort_by_key(|&m| {
                -results
                    .iter()
                    .find(|&&(rm, _)| rm == m)
                    .map_or(0, |&(_, s)| s)
            });
            if results.iter().any(|&(_, s)| s.abs() >= 5_000) {
                break;
            }
        }
        (
            (best_move_idx as usize % 8, best_move_idx as usize / 8),
            completed_depth,
        )
    }

    fn negamax(
        &self,
        board: &Board,
        player: Player,
        depth: usize,
        alpha: i32,
        beta: i32,
        abort: &AtomicBool,
    ) -> i32 {
        if abort.load(Ordering::Relaxed) {
            return 0;
        }
        let hash = compute_hash(board, player);
        let orig_alpha = alpha;
        let mut alpha = alpha;
        let mut tt_best: Option<u8> = None;

        if let Some(entry) = self.tt.probe(hash) {
            if entry.best_move < 64 {
                tt_best = Some(entry.best_move);
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
            self.tt.store(hash, 0, score, TT_EXACT, None);
            return score;
        }

        let moves_mask = board.valid_moves(player);
        if moves_mask.is_empty() {
            if board.valid_moves(player.opposite()).is_empty() {
                let score = (count_pieces(board, player.to_square_state())
                    - count_pieces(board, player.opposite().to_square_state()))
                    * 10_000;
                self.tt.store(hash, 255, score, TT_EXACT, None);
                return score;
            }
            return -self.negamax(board, player.opposite(), depth, -beta, -alpha, abort);
        }

        let mut moves = [0u8; 64];
        let mut move_count = 0usize;
        for m in moves_mask {
            moves[move_count] = m;
            move_count += 1;
        }
        let moves = &mut moves[..move_count];
        moves.sort_unstable_by_key(|&m| {
            if Some(m) == tt_best {
                i32::MIN
            } else {
                -move_priority(m)
            }
        });

        let mut best = i32::MIN + 1;
        let mut best_move = moves[0];
        for &m in moves.iter() {
            let b = board.make_move(m, player);
            let score = -self.negamax(&b, player.opposite(), depth - 1, -beta, -alpha, abort);
            if abort.load(Ordering::Relaxed) {
                return 0;
            }
            if score > best {
                best = score;
                best_move = m;
            }
            if score > alpha {
                alpha = score;
            }
            if alpha >= beta {
                break;
            }
        }

        let flag = if best >= beta {
            TT_LOWER
        } else if best > orig_alpha {
            TT_EXACT
        } else {
            TT_UPPER
        };
        self.tt.store(hash, depth as u8, best, flag, Some(best_move));
        best
    }
}

// ---------------------------------------------------------------------------
// User input
// ---------------------------------------------------------------------------

pub fn get_user_move(moves: &[(usize, usize)]) -> Option<(usize, usize)> {
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_moves() {
        let board = Board::new();
        let moves_mask = board.valid_moves(Player::BLACK);
        let expected_mask = (1u64 << 26) | (1u64 << 19) | (1u64 << 44) | (1u64 << 37);
        assert_eq!(moves_mask.0, expected_mask);
    }

    #[test]
    fn test_initial_flip() {
        let board = Board::new().make_move(26, Player::BLACK);
        assert_eq!(board.get(3, 3), SquareState::Black);
        assert_eq!(board.get(2, 3), SquareState::Black);
        assert_eq!(board.get(3, 4), SquareState::Black);
        assert_eq!(board.get(4, 3), SquareState::Black);
        assert_eq!(board.get(4, 4), SquareState::White);
    }
}
