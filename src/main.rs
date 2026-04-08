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
                _ => SquareState::Empty,
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
            _ => (0, 0),
        };

        update_board(&mut board, next_move.0, next_move.1, current_player);
        render_board(&board);

        current_player = match current_player {
            SquareState::White => SquareState::Black,
            SquareState::Black => SquareState::White,
            _ => SquareState::Empty,
        }
    }

    let white = count_pieces(&board, SquareState::White);
    let black = count_pieces(&board, SquareState::Black);
    if white > black {
        print!("White wins with {} pieces!\r\n", white);
    } else {
        match black > white {
            true => {
                print!("Black wins with {} pieces!\r\n", black);
            }
            false => {
                print!("Draw!\r\n");
            }
        }
    }
}

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

fn find_valid_moves(board: &Board, player: SquareState) -> Vec<(usize, usize)> {
    let mut moves: Vec<(usize, usize)> = Vec::new();
    let opponent = match player {
        SquareState::White => SquareState::Black,
        SquareState::Black => SquareState::White,
        _ => SquareState::Empty,
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

fn count_pieces(board: &Board, player: SquareState) -> usize {
    let mut player_count = 0;

    (0..BOARD_SIZE).for_each(|i| {
        for j in 0..BOARD_SIZE {
            if board[i][j] == player {
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
        print!("{} ", i);
        for j in 0..BOARD_SIZE {
            let symbol = match board[i][j] {
                SquareState::White => "\u{25cf} ", // black piece
                SquareState::Black => "\u{25cb} ", // white piece
                SquareState::Empty => "\u{00b7} ", // empty cell
            };
            print!("{}", symbol);
        }
        print!("{}\r\n", i);
    });
    print!("{}", termion::clear::CurrentLine);
    print!("  0 1 2 3 4 5 6 7\r\n");
    io::stdout().flush().unwrap();
}

const MAX_DEPTH: usize = 5;

fn opposite(player: SquareState) -> SquareState {
    match player {
        SquareState::White => SquareState::Black,
        SquareState::Black => SquareState::White,
        _ => SquareState::Empty,
    }
}

fn find_best_move(
    board: &Board,
    player: SquareState,
    moves: Vec<(usize, usize)>,
) -> (usize, usize) {
    let best_score = Arc::new(Mutex::new(i32::MIN));
    let best_move = Arc::new(Mutex::new(moves[0]));

    moves.par_iter().for_each(|&mov| {
        let score = score_move(board, player, player, mov, MAX_DEPTH);

        let mut best_score_guard = best_score.lock().unwrap();
        let mut best_move_guard = best_move.lock().unwrap();

        if score > *best_score_guard {
            *best_score_guard = score;
            *best_move_guard = mov;
        }
    });

    let result = *best_move.lock().unwrap();
    result
}

fn score_move(
    board: &Board,
    current_turn: SquareState,
    ai_player: SquareState,
    next_move: (usize, usize),
    depth: usize,
) -> i32 {
    let mut board_copy = board.to_owned();
    update_board(&mut board_copy, next_move.0, next_move.1, current_turn);

    if depth == 0 {
        return count_pieces(&board_copy, ai_player) as i32
            - count_pieces(&board_copy, opposite(ai_player)) as i32;
    }

    let next_turn = opposite(current_turn);
    let next_moves = find_valid_moves(&board_copy, next_turn);

    if next_moves.is_empty() {
        // next_turn must pass; check if current_turn can continue
        let same_moves = find_valid_moves(&board_copy, current_turn);
        if same_moves.is_empty() {
            // game over
            return count_pieces(&board_copy, ai_player) as i32
                - count_pieces(&board_copy, opposite(ai_player)) as i32;
        }
        // current_turn plays again
        if current_turn == ai_player {
            return same_moves
                .iter()
                .map(|&m| score_move(&board_copy, current_turn, ai_player, m, depth - 1))
                .max()
                .unwrap_or(0);
        } else {
            return same_moves
                .iter()
                .map(|&m| score_move(&board_copy, current_turn, ai_player, m, depth - 1))
                .min()
                .unwrap_or(0);
        }
    }

    if next_turn == ai_player {
        next_moves
            .iter()
            .map(|&m| score_move(&board_copy, next_turn, ai_player, m, depth - 1))
            .max()
            .unwrap_or(0)
    } else {
        next_moves
            .iter()
            .map(|&m| score_move(&board_copy, next_turn, ai_player, m, depth - 1))
            .min()
            .unwrap_or(0)
    }
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
