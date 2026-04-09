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
    (1, !A_FILE),  // Right
    (-1, !H_FILE), // Left
    (8, !0u64),    // Down
    (-8, !0u64),   // Up
    (7, !H_FILE),  // DownLeft
    (-7, !A_FILE), // UpRight
    (9, !A_FILE),  // DownRight
    (-9, !H_FILE), // UpLeft
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
        let mut frontier = candidates;
        let mut ray = candidates;
        for _ in 0..6 {
            let next = shift_mask(frontier, shift, mask);
            if (next & opponent) != 0 {
                frontier = next;
                ray |= frontier;
            } else {
                if (next & player) != 0 {
                    flips |= ray;
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
    let white = count_pieces(board, SquareState::White);
    let black = count_pieces(board, SquareState::Black);
    print!("\u{25cf} White: {white}  \u{25cb} Black: {black}\r\n");
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
    ) -> ((usize, usize), usize, i32) {
        let moves_mask = board.valid_moves(player);
        if moves_mask.is_empty() {
            return ((0, 0), 0, 0);
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
        let mut best_score = 0i32;

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
            if let Some(&(m, s)) = results.iter().max_by_key(|&&(_, s)| s) {
                best_move_idx = m;
                best_score = s;
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
            best_score,
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
        self.tt
            .store(hash, depth as u8, best, flag, Some(best_move));
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
    use super::{TranspositionTable, TT_EXACT, TT_LOWER, TT_SIZE, TT_UPPER};
    use std::time::Duration;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Parse an 8×8 board from a string of 'B', 'W', '.'. Whitespace is ignored.
    fn board_from_str(s: &str) -> Board {
        let mut board = Board::default();
        let chars: Vec<char> = s.chars().filter(|c| !c.is_whitespace()).collect();
        assert_eq!(
            chars.len(),
            64,
            "board string must have exactly 64 non-whitespace chars"
        );
        for (i, c) in chars.iter().enumerate() {
            let x = i % 8;
            let y = i / 8;
            match c {
                'B' => board.set(x, y, SquareState::Black),
                'W' => board.set(x, y, SquareState::White),
                '.' => {}
                _ => panic!("Invalid char '{c}' in board string"),
            }
        }
        board
    }

    fn moves_set(board: &Board, p: Player) -> std::collections::HashSet<(usize, usize)> {
        find_valid_moves(board, p).into_iter().collect()
    }

    // -----------------------------------------------------------------------
    // Original tests (preserved)
    // -----------------------------------------------------------------------

    #[test]
    fn test_initial_moves() {
        let board = Board::new();
        let moves_mask = board.valid_moves(Player::BLACK);
        let expected_mask = (1u64 << 26) | (1u64 << 19) | (1u64 << 44) | (1u64 << 37);
        assert_eq!(moves_mask.0, expected_mask);
    }

    #[test]
    fn test_multi_flip() {
        // Black at (0,0), whites at (1,0) and (2,0), black plays at (3,0).
        let mut board = Board::default();
        board.set(0, 0, SquareState::Black);
        board.set(1, 0, SquareState::White);
        board.set(2, 0, SquareState::White);
        let board = board.make_move(3, Player::BLACK);
        assert_eq!(board.get(1, 0), SquareState::Black);
        assert_eq!(board.get(2, 0), SquareState::Black);
        assert_eq!(board.get(3, 0), SquareState::Black);
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

    // -----------------------------------------------------------------------
    // Board primitives
    // -----------------------------------------------------------------------

    #[test]
    fn test_board_new_bit_layout() {
        let b = Board::new();
        assert_eq!(b.white, (1u64 << 27) | (1u64 << 36));
        assert_eq!(b.black, (1u64 << 28) | (1u64 << 35));
        assert_eq!(b.get(3, 3), SquareState::White);
        assert_eq!(b.get(4, 4), SquareState::White);
        assert_eq!(b.get(4, 3), SquareState::Black);
        assert_eq!(b.get(3, 4), SquareState::Black);
    }

    #[test]
    fn test_board_get_set_roundtrip() {
        let mut b = Board::default();
        for y in 0..8usize {
            for x in 0..8usize {
                b.set(x, y, SquareState::Black);
                assert_eq!(b.get(x, y), SquareState::Black, "({x},{y})");
                b.set(x, y, SquareState::White);
                assert_eq!(b.get(x, y), SquareState::White, "({x},{y})");
                b.set(x, y, SquareState::Empty);
                assert_eq!(b.get(x, y), SquareState::Empty, "({x},{y})");
            }
        }
    }

    #[test]
    fn test_board_set_clears_other_color() {
        let mut b = Board::default();
        b.set(3, 3, SquareState::Black);
        b.set(3, 3, SquareState::White);
        assert_eq!(b.get(3, 3), SquareState::White);
        assert_eq!(
            b.black & (1u64 << 27),
            0,
            "black bit should be cleared when set to white"
        );
    }

    #[test]
    fn test_player_opp_black() {
        let b = Board::new();
        let (p, o) = b.player_opp(Player::BLACK);
        assert_eq!(p, b.black);
        assert_eq!(o, b.white);
    }

    #[test]
    fn test_player_opp_white() {
        let b = Board::new();
        let (p, o) = b.player_opp(Player::WHITE);
        assert_eq!(p, b.white);
        assert_eq!(o, b.black);
    }

    #[test]
    fn test_player_methods() {
        assert_eq!(Player::BLACK.opposite(), Player::WHITE);
        assert_eq!(Player::WHITE.opposite(), Player::BLACK);
        assert_eq!(Player::BLACK.to_square_state(), SquareState::Black);
        assert_eq!(Player::WHITE.to_square_state(), SquareState::White);
    }

    // -----------------------------------------------------------------------
    // MoveMask iterator
    // -----------------------------------------------------------------------

    #[test]
    fn test_move_mask_is_empty() {
        assert!(MoveMask(0).is_empty());
        assert!(!MoveMask(1).is_empty());
        assert!(!MoveMask(u64::MAX).is_empty());
    }

    #[test]
    fn test_move_mask_iteration_ascending() {
        let mask = (1u64 << 5) | (1u64 << 17) | (1u64 << 63);
        let bits: Vec<u8> = MoveMask(mask).collect();
        assert_eq!(bits, vec![5, 17, 63]);
    }

    #[test]
    fn test_move_mask_count_matches_popcount() {
        let masks = [0u64, 1, 0xFFFF, 0x8000_0000_0000_0001, u64::MAX];
        for mask in masks {
            let count = MoveMask(mask).count();
            assert_eq!(count as u32, mask.count_ones(), "mask {mask:#x}");
        }
    }

    // -----------------------------------------------------------------------
    // Move generation — all 8 directions
    // -----------------------------------------------------------------------

    #[test]
    fn test_white_initial_moves() {
        // White has 4 symmetric moves matching black's
        let board = Board::new();
        let moves = board.valid_moves(Player::WHITE);
        let expected = (1u64 << 20) | (1u64 << 29) | (1u64 << 34) | (1u64 << 43);
        assert_eq!(moves.0, expected);
    }

    #[test]
    fn test_no_moves_when_boxed_out() {
        let mut b = Board::default();
        for y in 0..8usize {
            for x in 0..8usize {
                b.set(x, y, SquareState::White);
            }
        }
        assert!(b.valid_moves(Player::BLACK).is_empty());
    }

    #[test]
    fn test_move_direction_right() {
        // Black at (0,0), white at (1,0) → move at (2,0)=bit2
        let mut b = Board::default();
        b.set(0, 0, SquareState::Black);
        b.set(1, 0, SquareState::White);
        assert!(b.valid_moves(Player::BLACK).0 & (1u64 << 2) != 0);
    }

    #[test]
    fn test_move_direction_left() {
        // Black at (7,0), white at (6,0) → move at (5,0)=bit5
        let mut b = Board::default();
        b.set(7, 0, SquareState::Black);
        b.set(6, 0, SquareState::White);
        assert!(b.valid_moves(Player::BLACK).0 & (1u64 << 5) != 0);
    }

    #[test]
    fn test_move_direction_down() {
        // Black at (0,0), white at (0,1) → move at (0,2)=bit16
        let mut b = Board::default();
        b.set(0, 0, SquareState::Black);
        b.set(0, 1, SquareState::White);
        assert!(b.valid_moves(Player::BLACK).0 & (1u64 << 16) != 0);
    }

    #[test]
    fn test_move_direction_up() {
        // Black at (0,7), white at (0,6) → move at (0,5)=bit40
        let mut b = Board::default();
        b.set(0, 7, SquareState::Black);
        b.set(0, 6, SquareState::White);
        assert!(b.valid_moves(Player::BLACK).0 & (1u64 << 40) != 0);
    }

    #[test]
    fn test_move_direction_down_right() {
        // Black at (0,0), white at (1,1) → move at (2,2)=bit18
        let mut b = Board::default();
        b.set(0, 0, SquareState::Black);
        b.set(1, 1, SquareState::White);
        assert!(b.valid_moves(Player::BLACK).0 & (1u64 << 18) != 0);
    }

    #[test]
    fn test_move_direction_up_left() {
        // Black at (7,7), white at (6,6) → move at (5,5)=bit45
        let mut b = Board::default();
        b.set(7, 7, SquareState::Black);
        b.set(6, 6, SquareState::White);
        assert!(b.valid_moves(Player::BLACK).0 & (1u64 << 45) != 0);
    }

    #[test]
    fn test_move_direction_down_left() {
        // Black at (7,0)=bit7, white at (6,1)=bit14 → move at (5,2)=bit21
        let mut b = Board::default();
        b.set(7, 0, SquareState::Black);
        b.set(6, 1, SquareState::White);
        assert!(b.valid_moves(Player::BLACK).0 & (1u64 << 21) != 0);
    }

    #[test]
    fn test_move_direction_up_right() {
        // Black at (0,7)=bit56, white at (1,6)=bit49 → move at (2,5)=bit42
        let mut b = Board::default();
        b.set(0, 7, SquareState::Black);
        b.set(1, 6, SquareState::White);
        assert!(b.valid_moves(Player::BLACK).0 & (1u64 << 42) != 0);
    }

    // -----------------------------------------------------------------------
    // Flip logic
    // -----------------------------------------------------------------------

    #[test]
    fn test_flip_no_anchor_means_no_valid_move() {
        // Whites exist but no black anchor — (0,0) should not be a valid move
        let mut b = Board::default();
        b.set(1, 0, SquareState::White);
        b.set(2, 0, SquareState::White);
        assert_eq!(b.valid_moves(Player::BLACK).0 & 1, 0);
    }

    #[test]
    fn test_flip_multi_directional() {
        // Play at (3,3)=bit27: whites in all 4 cardinal directions with black anchors beyond
        let mut b = Board::default();
        b.set(4, 3, SquareState::White);
        b.set(5, 3, SquareState::Black); // right
        b.set(2, 3, SquareState::White);
        b.set(1, 3, SquareState::Black); // left
        b.set(3, 4, SquareState::White);
        b.set(3, 5, SquareState::Black); // down
        b.set(3, 2, SquareState::White);
        b.set(3, 1, SquareState::Black); // up
        let b = b.make_move(27, Player::BLACK);
        assert_eq!(b.get(3, 3), SquareState::Black);
        assert_eq!(b.get(4, 3), SquareState::Black);
        assert_eq!(b.get(2, 3), SquareState::Black);
        assert_eq!(b.get(3, 4), SquareState::Black);
        assert_eq!(b.get(3, 2), SquareState::Black);
        assert_eq!(b.get(5, 3), SquareState::Black); // anchors unchanged
        assert_eq!(b.get(1, 3), SquareState::Black);
    }

    #[test]
    fn test_flip_long_chain() {
        // Black at (0,0), 5 whites in a row, play at (6,0)=bit6 flips all 5
        let mut b = Board::default();
        b.set(0, 0, SquareState::Black);
        for x in 1..=5 {
            b.set(x, 0, SquareState::White);
        }
        let b = b.make_move(6, Player::BLACK);
        for x in 0..=6 {
            assert_eq!(b.get(x, 0), SquareState::Black, "col {x} should be black");
        }
    }

    #[test]
    fn test_flip_corner_capture() {
        // Anchor at (2,0), white at (1,0), play at corner (0,0)=bit0
        let mut b = Board::default();
        b.set(2, 0, SquareState::Black);
        b.set(1, 0, SquareState::White);
        let b = b.make_move(0, Player::BLACK);
        assert_eq!(b.get(0, 0), SquareState::Black);
        assert_eq!(b.get(1, 0), SquareState::Black);
        assert_eq!(b.get(2, 0), SquareState::Black);
    }

    #[test]
    fn test_flip_opponent_loses_flipped_bits() {
        // Verify opponent bitboard is updated when pieces flip
        let board = Board::new().make_move(26, Player::BLACK);
        // (3,3)=bit27 was white, is now black
        assert_eq!(board.white & (1u64 << 27), 0, "bit 27 removed from white");
        assert_ne!(board.black & (1u64 << 27), 0, "bit 27 added to black");
    }

    // -----------------------------------------------------------------------
    // Column-wrap prevention
    // -----------------------------------------------------------------------

    #[test]
    fn test_no_wrap_right_column_boundary() {
        // Col 7 row 0 (bit 7) and col 0 row 1 (bit 8) are bit-adjacent but not board-adjacent.
        // Black at (7,0), white at (0,1) must NOT generate a move at (1,1)=bit9.
        let mut b = Board::default();
        b.set(7, 0, SquareState::Black);
        b.set(0, 1, SquareState::White);
        assert_eq!(b.valid_moves(Player::BLACK).0 & (1u64 << 9), 0);
    }

    #[test]
    fn test_no_flip_across_column_boundary() {
        // Playing at (1,1)=bit9 must not flip white at (0,1) via the phantom right-wrap path.
        let mut b = Board::default();
        b.set(7, 0, SquareState::Black);
        b.set(0, 1, SquareState::White);
        let b2 = b.make_move(9, Player::BLACK);
        assert_eq!(b2.get(0, 1), SquareState::White);
    }

    // -----------------------------------------------------------------------
    // find_valid_moves / count_pieces
    // -----------------------------------------------------------------------

    #[test]
    fn test_find_valid_moves_matches_bitmask() {
        let board = Board::new();
        let moves = find_valid_moves(&board, Player::BLACK);
        let mask = board.valid_moves(Player::BLACK);
        assert_eq!(moves.len() as u32, mask.0.count_ones());
        for (x, y) in &moves {
            assert!(
                mask.0 & (1u64 << (y * 8 + x)) != 0,
                "({x},{y}) not in bitmask"
            );
        }
    }

    #[test]
    fn test_count_pieces_initial_board() {
        let b = Board::new();
        assert_eq!(count_pieces(&b, SquareState::Black), 2);
        assert_eq!(count_pieces(&b, SquareState::White), 2);
        assert_eq!(count_pieces(&b, SquareState::Empty), 60);
    }

    #[test]
    fn test_count_pieces_always_sums_to_64() {
        let boards = [
            Board::new(),
            Board::default(),
            Board::new().make_move(26, Player::BLACK),
        ];
        for b in &boards {
            let total = count_pieces(b, SquareState::Black)
                + count_pieces(b, SquareState::White)
                + count_pieces(b, SquareState::Empty);
            assert_eq!(total, 64);
        }
    }

    #[test]
    fn test_count_pieces_empty_board() {
        let b = Board::default();
        assert_eq!(count_pieces(&b, SquareState::Black), 0);
        assert_eq!(count_pieces(&b, SquareState::White), 0);
        assert_eq!(count_pieces(&b, SquareState::Empty), 64);
    }

    // -----------------------------------------------------------------------
    // board_from_str helper sanity check
    // -----------------------------------------------------------------------

    #[test]
    fn test_board_from_str() {
        let b = board_from_str(
            "........\
             ........\
             ........\
             ...WB...\
             ...BW...\
             ........\
             ........\
             ........",
        );
        assert_eq!(b.get(3, 3), SquareState::White);
        assert_eq!(b.get(4, 3), SquareState::Black);
        assert_eq!(b.get(3, 4), SquareState::Black);
        assert_eq!(b.get(4, 4), SquareState::White);
        assert_eq!(count_pieces(&b, SquareState::Black), 2);
        assert_eq!(count_pieces(&b, SquareState::White), 2);
    }

    // -----------------------------------------------------------------------
    // Evaluation
    // -----------------------------------------------------------------------

    #[test]
    fn test_evaluate_initial_is_zero() {
        let b = Board::new();
        assert_eq!(evaluate(&b, Player::BLACK), 0);
        assert_eq!(evaluate(&b, Player::WHITE), 0);
    }

    #[test]
    fn test_evaluate_perspective_is_negated() {
        // evaluate(board, BLACK) == -evaluate(board, WHITE) for any position
        let board = Board::new().make_move(26, Player::BLACK);
        assert_eq!(
            evaluate(&board, Player::BLACK),
            -evaluate(&board, Player::WHITE)
        );
    }

    #[test]
    fn test_evaluate_endgame_branch_triggered() {
        // 52 pieces placed → 12 empty → endgame scoring activates
        let mut b = Board::default();
        let mut count = 0usize;
        'outer: for y in 0..8usize {
            for x in 0..8usize {
                if count >= 52 {
                    break 'outer;
                }
                if count < 30 {
                    b.set(x, y, SquareState::Black);
                } else {
                    b.set(x, y, SquareState::White);
                }
                count += 1;
            }
        }
        assert_eq!(count_pieces(&b, SquareState::Empty), 12);
        let p = count_pieces(&b, SquareState::Black);
        let o = count_pieces(&b, SquareState::White);
        assert_eq!(evaluate(&b, Player::BLACK), (p - o) * 500);
        assert_eq!(evaluate(&b, Player::WHITE), (o - p) * 500);
    }

    #[test]
    fn test_evaluate_endgame_symmetric_is_zero() {
        // Equal pieces at endgame depth → score of 0
        let mut b = Board::default();
        let mut count = 0usize;
        'outer: for y in 0..8usize {
            for x in 0..8usize {
                if count >= 52 {
                    break 'outer;
                }
                if count % 2 == 0 {
                    b.set(x, y, SquareState::Black);
                } else {
                    b.set(x, y, SquareState::White);
                }
                count += 1;
            }
        }
        assert_eq!(evaluate(&b, Player::BLACK), 0);
    }

    // -----------------------------------------------------------------------
    // Zobrist hashing
    // -----------------------------------------------------------------------

    #[test]
    fn test_hash_deterministic() {
        let b = Board::new();
        assert_eq!(
            compute_hash(&b, Player::BLACK),
            compute_hash(&b, Player::BLACK)
        );
        assert_eq!(
            compute_hash(&b, Player::WHITE),
            compute_hash(&b, Player::WHITE)
        );
    }

    #[test]
    fn test_hash_player_changes_hash() {
        let b = Board::new();
        assert_ne!(
            compute_hash(&b, Player::BLACK),
            compute_hash(&b, Player::WHITE)
        );
    }

    #[test]
    fn test_hash_different_positions_differ() {
        // Each of the 4 black opening moves produces a distinct board and hash
        let boards = [
            Board::new(),
            Board::new().make_move(26, Player::BLACK),
            Board::new().make_move(19, Player::BLACK),
            Board::new().make_move(37, Player::BLACK),
            Board::new().make_move(44, Player::BLACK),
        ];
        for i in 0..boards.len() {
            for j in (i + 1)..boards.len() {
                assert_ne!(
                    compute_hash(&boards[i], Player::BLACK),
                    compute_hash(&boards[j], Player::BLACK),
                    "boards {i} and {j} should hash differently"
                );
            }
        }
    }

    #[test]
    fn test_hash_empty_board_consistent() {
        let b = Board::default();
        assert_eq!(
            compute_hash(&b, Player::BLACK),
            compute_hash(&b, Player::BLACK)
        );
        assert_ne!(
            compute_hash(&b, Player::BLACK),
            compute_hash(&b, Player::WHITE)
        );
    }

    // -----------------------------------------------------------------------
    // Transposition table
    // -----------------------------------------------------------------------

    #[test]
    fn test_tt_store_and_probe_exact() {
        let tt = TranspositionTable::new(TT_SIZE);
        let hash = 0xdeadbeef_cafebabe_u64;
        assert!(tt.probe(hash).is_none());
        tt.store(hash, 5, 42, TT_EXACT, Some(15));
        let e = tt.probe(hash).unwrap();
        assert_eq!(e.score, 42);
        assert_eq!(e.flag, TT_EXACT);
        assert_eq!(e.depth, 5);
        assert_eq!(e.best_move, 15);
    }

    #[test]
    fn test_tt_probe_miss() {
        let tt = TranspositionTable::new(TT_SIZE);
        assert!(tt.probe(0x1234_5678_9abc_def0_u64).is_none());
    }

    #[test]
    fn test_tt_flags_stored_correctly() {
        let tt = TranspositionTable::new(TT_SIZE);
        tt.store(0xaa, 4, 100, TT_EXACT, Some(0));
        tt.store(0xbb, 4, 200, TT_LOWER, Some(1));
        tt.store(0xcc, 4, -100, TT_UPPER, Some(2));
        assert_eq!(tt.probe(0xaa).unwrap().flag, TT_EXACT);
        assert_eq!(tt.probe(0xbb).unwrap().flag, TT_LOWER);
        assert_eq!(tt.probe(0xcc).unwrap().flag, TT_UPPER);
    }

    #[test]
    fn test_tt_best_move_none_stored_as_255() {
        let tt = TranspositionTable::new(TT_SIZE);
        tt.store(0x111, 3, 10, TT_LOWER, None);
        assert_eq!(tt.probe(0x111).unwrap().best_move, 255);
    }

    // -----------------------------------------------------------------------
    // Search correctness
    // -----------------------------------------------------------------------

    #[test]
    fn test_search_depth_0_equals_evaluate() {
        let engine = Engine::new();
        let board = Board::new();
        assert_eq!(
            engine.search_fixed_depth(&board, Player::BLACK, 0),
            evaluate(&board, Player::BLACK)
        );
    }

    #[test]
    fn test_search_deterministic() {
        let engine = Engine::new();
        let board = Board::new();
        let s1 = engine.search_fixed_depth(&board, Player::BLACK, 4);
        let s2 = engine.search_fixed_depth(&board, Player::BLACK, 4);
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_search_initial_score_is_symmetric() {
        // Opening position is symmetric → both players score equally at any depth
        let engine = Engine::new();
        let board = Board::new();
        assert_eq!(
            engine.search_fixed_depth(&board, Player::BLACK, 3),
            engine.search_fixed_depth(&board, Player::WHITE, 3)
        );
    }

    #[test]
    fn test_negamax_terminal_draw() {
        // Full board, 32 black + 32 white → score 0 from either side
        let mut b = Board::default();
        for y in 0..8usize {
            for x in 0..8usize {
                if (x + y) % 2 == 0 {
                    b.set(x, y, SquareState::Black);
                } else {
                    b.set(x, y, SquareState::White);
                }
            }
        }
        let engine = Engine::new();
        assert_eq!(engine.search_fixed_depth(&b, Player::BLACK, 1), 0);
    }

    #[test]
    fn test_negamax_terminal_win() {
        // Full board, 40 black + 24 white → terminal score (40-24)*10_000 = 160_000
        let mut b = Board::default();
        let mut count = 0usize;
        for y in 0..8usize {
            for x in 0..8usize {
                if count < 40 {
                    b.set(x, y, SquareState::Black);
                } else {
                    b.set(x, y, SquareState::White);
                }
                count += 1;
            }
        }
        let engine = Engine::new();
        assert_eq!(engine.search_fixed_depth(&b, Player::BLACK, 1), 160_000);
    }

    #[test]
    fn test_search_endgame_score_at_depth_0() {
        // 55 squares filled → 9 empty (< 12), endgame branch: score = (35-20)*500 = 7500
        let mut b = Board::default();
        let mut count = 0usize;
        'outer: for y in 0..8usize {
            for x in 0..8usize {
                if count >= 55 {
                    break 'outer;
                }
                if count < 35 {
                    b.set(x, y, SquareState::Black);
                } else {
                    b.set(x, y, SquareState::White);
                }
                count += 1;
            }
        }
        let engine = Engine::new();
        assert_eq!(engine.search_fixed_depth(&b, Player::BLACK, 0), 7500);
        assert_eq!(engine.search_fixed_depth(&b, Player::WHITE, 0), -7500);
    }

    #[test]
    fn test_find_best_move_returns_valid_move() {
        let engine = Engine::new();
        let board = Board::new();
        let valid = moves_set(&board, Player::BLACK);
        let (mv, depth, _score) = engine.find_best_move(&board, Player::BLACK, Duration::from_millis(200));
        assert!(valid.contains(&mv), "best move {mv:?} must be a valid move");
        assert!(depth > 0, "should complete at least depth 1");
    }

    // -----------------------------------------------------------------------
    // Full game simulation
    // -----------------------------------------------------------------------

    #[test]
    fn test_full_game_no_panic() {
        // Play through a complete game picking the first available move each turn.
        let mut board = Board::new();
        let mut player = Player::BLACK;
        let mut consecutive_passes = 0;
        let mut move_count = 0usize;

        loop {
            let moves = board.valid_moves(player);
            if moves.is_empty() {
                consecutive_passes += 1;
                if consecutive_passes >= 2 {
                    break;
                }
            } else {
                consecutive_passes = 0;
                let m = board.valid_moves(player).next().unwrap();
                board = board.make_move(m, player);
                move_count += 1;
            }
            player = player.opposite();
            assert!(move_count <= 64, "game must end within 64 moves");
        }

        let total = count_pieces(&board, SquareState::Black)
            + count_pieces(&board, SquareState::White)
            + count_pieces(&board, SquareState::Empty);
        assert_eq!(total, 64);
    }

    #[test]
    fn test_game_over_when_both_pass() {
        // A completely filled board → both players immediately have no moves
        let mut b = Board::default();
        b.set(0, 0, SquareState::Black);
        for y in 0..8usize {
            for x in 0..8usize {
                if b.get(x, y) == SquareState::Empty {
                    b.set(x, y, SquareState::White);
                }
            }
        }
        assert!(b.valid_moves(Player::BLACK).is_empty());
        assert!(b.valid_moves(Player::WHITE).is_empty());
    }
}
