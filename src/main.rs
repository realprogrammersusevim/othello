use rayon::prelude::*;
use std::io::{self, stdin, stdout, Write};
use std::sync::{Arc, Mutex};
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
    stdout().into_raw_mode().unwrap();

    let mut board: Board = [[SquareState::Empty; BOARD_SIZE]; BOARD_SIZE];

    // Set up initial state
    board[3][3] = SquareState::White;
    board[3][4] = SquareState::Black;
    board[4][3] = SquareState::Black;
    board[4][4] = SquareState::White;

    update_board(&mut board, 2, 3, SquareState::Black);
    render_board(&board);
    let mut current_player = SquareState::White;
    for _ in 1..60 {
        let moves = find_valid_moves(&board, current_player);
        let next_move: (usize, usize) = match current_player {
            SquareState::White => get_user_move(moves),
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
        println!("White wins with {} pieces!", white);
    } else {
        match black > white {
            true => {
                println!("Black wins with {} pieces!", black);
            }
            false => {
                println!("Draw!");
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
    println!("  0 1 2 3 4 5 6 7");
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
        println!("{}", i);
    });
    print!("{}", termion::clear::CurrentLine);
    println!("  0 1 2 3 4 5 6 7");
    io::stdout().flush().unwrap();
}

const MAX_DEPTH: usize = 12;

fn find_best_move(
    board: &Board,
    player: SquareState,
    moves: Vec<(usize, usize)>,
) -> (usize, usize) {
    // TODO: implement alpha-beta search pruning so we can avoid spending time on
    // bad branches and search the good branches deeper.
    let best_score = Arc::new(Mutex::new(i32::MIN));
    let best_move = Arc::new(Mutex::new(moves[0]));

    moves.par_iter().for_each(|&mov| {
        let score = score_move(
            board,
            player,
            mov,
            MAX_DEPTH,
            &mut best_score.lock().unwrap().clone(),
        );

        let mut best_score_guard = best_score.lock().unwrap();
        let mut best_move_guard = best_move.lock().unwrap();

        if score > *best_score_guard {
            *best_score_guard = score;
            *best_move_guard = mov;
        }
    });

    let x = *best_move.lock().unwrap();
    x
}

fn score_move(
    board: &Board,
    player: SquareState,
    next_move: (usize, usize),
    depth: usize,
    best_score: &mut i32,
) -> i32 {
    if depth == 0 {
        return count_pieces(board, player) as i32;
    }

    let mut board_copy = board.to_owned();
    update_board(&mut board_copy, next_move.0, next_move.1, player);
    let next_valid_moves = find_valid_moves(&board_copy, player);

    let mut current_score = 0;
    for next_next_move in next_valid_moves {
        current_score += score_move(&board_copy, player, next_next_move, depth - 1, best_score);
    }

    if depth == MAX_DEPTH && current_score > *best_score {
        *best_score = current_score;
        return current_score;
    }

    current_score
}

fn get_user_move(moves: Vec<(usize, usize)>) -> (usize, usize) {
    let labels = vec![
        "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "q", "w", "e", "r", "t", "y", "u", "i",
        "o", "p", "a", "s", "d", "f",
    ];

    for i in 0..moves.len() {
        print!("{}", termion::clear::CurrentLine);
        println!("{}: ({}, {})  ", labels[i], moves[i].0, moves[i].1);
    }
    stdout().flush().unwrap();

    loop {
        for event in stdin().events() {
            if let Event::Key(key) = event.unwrap() {
                match key {
                    Key::Char('0') => return moves[0],
                    Key::Char('1') => return moves[1],
                    Key::Char('2') => return moves[2],
                    Key::Char('3') => return moves[3],
                    Key::Char('4') => return moves[4],
                    Key::Char('5') => return moves[5],
                    Key::Char('6') => return moves[6],
                    Key::Char('7') => return moves[7],
                    Key::Char('8') => return moves[8],
                    Key::Char('9') => return moves[9],
                    Key::Char('q') => return moves[10],
                    Key::Char('w') => return moves[11],
                    Key::Char('e') => return moves[12],
                    Key::Char('r') => return moves[13],
                    Key::Char('t') => return moves[14],
                    Key::Char('y') => return moves[15],
                    Key::Char('u') => return moves[16],
                    Key::Char('i') => return moves[17],
                    Key::Char('o') => return moves[18],
                    Key::Char('p') => return moves[19],
                    Key::Char('a') => return moves[20],
                    Key::Char('s') => return moves[21],
                    Key::Char('d') => return moves[22],
                    Key::Char('f') => return moves[23],
                    _ => continue,
                }
            }
        }
    }
}
