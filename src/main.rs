use othello::*;
use std::io::{self, stdout, Write};
use std::time::Duration;
use termion::raw::IntoRawMode;

const TIME_LIMIT: Duration = Duration::from_secs(10);

fn main() {
    print!("{}", termion::clear::All);
    let _raw = stdout().into_raw_mode().unwrap();

    let mut board = Board::new();

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
            SquareState::Black => find_best_move(&board, current_player, moves.clone(), TIME_LIMIT),
            SquareState::Empty => ((0, 0), 0),
        };

        update_board(
            &mut board,
            (next_move.1 * 8 + next_move.0) as u8,
            current_player,
        );
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
