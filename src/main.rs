use std::io::{self, stdin, stdout, Write};
use termion::event::{Event, Key};
use termion::input::TermRead;
use termion::raw::IntoRawMode;

const BOARD_SIZE: usize = 8;

#[derive(Copy, Clone, PartialEq)]
enum Square {
    Empty,
    White,
    Black,
}

type Board = [[Square; BOARD_SIZE]; BOARD_SIZE];

fn main() {
    print!("{}", termion::clear::All);
    stdout().into_raw_mode().unwrap();

    let mut board: Board = [[Square::Empty; BOARD_SIZE]; BOARD_SIZE];

    // Set up initial state
    board[3][3] = Square::White;
    board[3][4] = Square::Black;
    board[4][3] = Square::Black;
    board[4][4] = Square::White;

    update_board(&mut board, 2, 3, Square::Black);
    render_board(&board);
    let mut current_player = Square::White;
    for _ in 1..60 {
        let moves = find_valid_moves(&board, current_player);
        let mut next_move: (usize, usize) = (0, 0);
        if current_player == Square::Black {
            next_move = find_best_move(&board, current_player, moves.clone());
        } else if current_player == Square::White {
            next_move = get_user_move(moves);
        }

        update_board(&mut board, next_move.0, next_move.1, current_player);
        render_board(&board);

        current_player = match current_player {
            Square::White => Square::Black,
            Square::Black => Square::White,
            _ => Square::Empty,
        }
    }

    let white = count_pieces(&board, Square::White);
    let black = count_pieces(&board, Square::Black);
    if white > black {
        println!("White wins with {} pieces!", white);
    } else if black > white {
        println!("Black wins with {} pieces!", black);
    } else {
        println!("Draw!");
    }
}

fn update_board(board: &mut Board, x: usize, y: usize, player: Square) {
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
                if board[k][l] == Square::Empty {
                    break;
                }
            }
        }
    }
}

fn find_valid_moves(board: &Board, player: Square) -> Vec<(usize, usize)> {
    let mut moves: Vec<(usize, usize)> = Vec::new();
    let opponent = match player {
        Square::White => Square::Black,
        Square::Black => Square::White,
        _ => Square::Empty,
    };

    for i in 0..BOARD_SIZE {
        for j in 0..BOARD_SIZE {
            if board[i][j] != Square::Empty {
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

fn count_pieces(board: &Board, player: Square) -> usize {
    let mut player_count = 0;

    for i in 0..BOARD_SIZE {
        for j in 0..BOARD_SIZE {
            if board[i][j] == player {
                player_count += 1;
            }
        }
    }

    player_count
}

fn render_board(board: &Board) {
    print!("{}", termion::cursor::Goto(1, 1));
    print!("{}", termion::clear::CurrentLine);
    println!("  0 1 2 3 4 5 6 7");
    for i in 0..BOARD_SIZE {
        print!("{}", termion::clear::CurrentLine);
        print!("{} ", i);
        for j in 0..BOARD_SIZE {
            let symbol = match board[i][j] {
                Square::White => "\u{25cf} ", // black piece
                Square::Black => "\u{25cb} ", // white piece
                Square::Empty => "\u{00b7} ", // empty cell
            };
            print!("{}", symbol);
        }
        println!("{}", i);
    }
    print!("{}", termion::clear::CurrentLine);
    println!("  0 1 2 3 4 5 6 7");
    io::stdout().flush().unwrap();
}

fn find_best_move(board: &Board, player: Square, moves: Vec<(usize, usize)>) -> (usize, usize) {
    let mut best_score = 0;
    let mut best_move: (usize, usize) = moves[0];

    for one_valid in moves {
        let mut current_score = 0;
        let mut board_copy = board.to_owned();
        update_board(&mut board_copy, one_valid.1, one_valid.0, player);
        // current_score += count_pieces(&board_copy, player);

        for two_valid in find_valid_moves(&board_copy, player) {
            let mut board_copy2 = board_copy.to_owned();
            update_board(&mut board_copy2, two_valid.1, two_valid.0, player);
            // current_score += count_pieces(&board_copy2, player);

            for three_valid in find_valid_moves(&board_copy2, player) {
                let mut board_copy3 = board_copy2.to_owned();
                update_board(&mut board_copy3, three_valid.1, three_valid.0, player);
                // current_score += count_pieces(&board_copy3, player);

                for four in find_valid_moves(&board_copy3, player) {
                    let mut board_copy4 = board_copy3.to_owned();
                    update_board(&mut board_copy4, four.1, four.0, player);
                    // current_score += count_pieces(&board_copy4, player);

                    for five in find_valid_moves(&board_copy4, player) {
                        let mut board_copy5 = board_copy4.to_owned();
                        update_board(&mut board_copy5, five.1, five.0, player);
                        // current_score += count_pieces(&board_copy5, player);

                        for six in find_valid_moves(&board_copy5, player) {
                            let mut board_copy6 = board_copy5.to_owned();
                            update_board(&mut board_copy6, six.1, six.0, player);
                            // current_score += count_pieces(&board_copy6, player);

                            for seven in find_valid_moves(&board_copy6, player) {
                                let mut board_copy7 = board_copy6.to_owned();
                                update_board(&mut board_copy7, seven.1, seven.0, player);
                                // current_score += count_pieces(&board_copy7, player);

                                for eight in find_valid_moves(&board_copy7, player) {
                                    let mut board_copy8 = board_copy7.to_owned();
                                    update_board(&mut board_copy8, eight.0, eight.1, player);
                                    current_score += count_pieces(&board_copy7, player);
                                }
                            }
                        }
                    }
                }
            }
        }

        if current_score > best_score {
            best_move = one_valid;
            best_score = current_score;
        }
    }

    best_move
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
            match event.unwrap() {
                Event::Key(key) => match key {
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
                },
                _ => {}
            }
        }
    }
}
