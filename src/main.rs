use rayon::prelude::*;
use std::io::{self, stdin, stdout, Write};
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

const MAX_DEPTH: usize = 14;

fn opposite(player: SquareState) -> SquareState {
    match player {
        SquareState::White => SquareState::Black,
        SquareState::Black => SquareState::White,
        SquareState::Empty => SquareState::Empty,
    }
}

// Higher = better move to try first; improves alpha-beta cut rate
fn move_priority(mov: (usize, usize)) -> i32 {
    let (x, y) = mov;
    if (x == 0 || x == 7) && (y == 0 || y == 7) {
        return 3; // corner
    }
    if (x == 1 || x == 6) && (y == 1 || y == 6) {
        return -2; // X-square — adjacent to corner, usually bad
    }
    if x == 0 || x == 7 || y == 0 || y == 7 {
        return 1; // edge
    }
    0
}

fn find_best_move(
    board: &Board,
    player: SquareState,
    mut moves: Vec<(usize, usize)>,
) -> (usize, usize) {
    moves.sort_by_key(|&m| -move_priority(m));

    // Parallel over top-level moves; each thread runs its own alpha-beta subtree
    moves
        .par_iter()
        .map(|&mov| {
            let mut b = board.to_owned();
            update_board(&mut b, mov.0, mov.1, player);
            let score = -negamax(&b, opposite(player), MAX_DEPTH - 1, i32::MIN + 1, i32::MAX);
            (mov, score)
        })
        .max_by_key(|t| t.1)
        .map_or(moves[0], |t| t.0)
}

fn negamax(board: &Board, player: SquareState, depth: usize, alpha: i32, beta: i32) -> i32 {
    if depth == 0 {
        return count_pieces(board, player) - count_pieces(board, opposite(player));
    }

    let mut moves = find_valid_moves(board, player);
    if moves.is_empty() {
        let opp_moves = find_valid_moves(board, opposite(player));
        if opp_moves.is_empty() {
            // game over
            return count_pieces(board, player) - count_pieces(board, opposite(player));
        }
        // forced pass
        return -negamax(board, opposite(player), depth - 1, -beta, -alpha);
    }

    moves.sort_by_key(|&m| -move_priority(m));

    // Second parallel level: fans out ~moves² tasks so all cores stay busy.
    // Alpha-beta can't be shared across threads, so we use a full window here;
    // sequential alpha-beta takes over one level below.
    if depth >= MAX_DEPTH - 1 {
        return moves
            .par_iter()
            .map(|&mov| {
                let mut b = board.to_owned();
                update_board(&mut b, mov.0, mov.1, player);
                -negamax(&b, opposite(player), depth - 1, i32::MIN + 1, i32::MAX)
            })
            .max()
            .unwrap_or(i32::MIN + 1);
    }

    // Sequential alpha-beta for the rest of the tree
    let mut alpha = alpha;
    let mut best = i32::MIN + 1;
    for mov in moves {
        let mut b = board.to_owned();
        update_board(&mut b, mov.0, mov.1, player);
        let score = -negamax(&b, opposite(player), depth - 1, -beta, -alpha);
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
