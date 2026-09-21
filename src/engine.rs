pub mod board;
use board::*;

pub mod attacks;

pub struct Config {
    threads: usize,
}

pub struct Engine {
    config: Config,
    board: Board,
}

impl Engine {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            board: Board::new(),
        }
    }

    pub fn newgame(&mut self) {
        self.board = Board::new();
    }

    pub fn position_startpos(&mut self) {
        self.board.position_startpos();
    }

    pub fn position_startpos_moves(&mut self, moves: &[&str]) {
        self.board.position_startpos_moves(moves);
    }

    pub fn position_fen(&mut self, fen: &[&str]) {
        self.board.position_fen(fen);
    }

    pub fn go(&mut self) {
        let moves = self.board.generate_moves();
        let mv = moves[rand::random_range(..moves.len())];
        println!("bestmove {}", mv);
    }
}

impl Config {
    pub fn new(threads: usize) -> Self {
        Self { threads }
    }
}
