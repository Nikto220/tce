use std::time::Instant;

pub mod board;
use board::*;

pub mod attacks;

mod eval;

const MAX_DEPTH: usize = 128;

pub struct Config {
    threads: usize,
}

pub struct Engine {
    config: Config,
    board: Board,
    nodes: u64,
    pv: [[Option<Move>; MAX_DEPTH]; MAX_DEPTH],
    pv_len: [usize; MAX_DEPTH],
}

impl Engine {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            board: Board::new(),
            nodes: 0,
            pv: [[None; MAX_DEPTH]; MAX_DEPTH],
            pv_len: [0; MAX_DEPTH],
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

    pub fn random_move(&mut self) {
        let moves = self.board.generate_moves();
        if moves.is_empty() {return;}
        let mv = moves[rand::random_range(..moves.len())];
        println!("bestmove {}", mv);
    }

    pub fn go_depth(&mut self, depth: usize) {
        let mut bestmove = None;
        let mut score;
        let start = Instant::now();

        for i in 0..depth {
            (bestmove, score) = self.search(i + 1);

            let time = start.elapsed().as_millis();
            let nps = if time > 0 {
                self.nodes * 1000 / time as u64
            } else {
                0
            };


            if score >= eval::MATE - (i + 1) as i32 {
                let mate_ply = eval::MATE - score;
                let mate_moves = (mate_ply + 1) / 2;

                print!(
                    "info depth {} score mate {} nodes {} nps {} time {} pv ",
                    i + 1,
                    mate_moves,
                    self.nodes,
                    nps,
                    time
                );

                for i in 0..self.pv_len[0] {
                    print!("{} ", self.pv[0][i].unwrap());
                }

                println!();
            } else if score <= -eval::MATE + (i + 1) as i32 {
                let mate_ply = eval::MATE + score;
                let mate_moves = (mate_ply + 1) / 2;

                print!(
                    "info depth {} score mate -{} nodes {} nps {} time {} pv ",
                    i + 1,
                    mate_moves,
                    self.nodes,
                    nps,
                    time
                );

                for i in 0..self.pv_len[0] {
                    print!("{} ", self.pv[0][i].unwrap());
                }

                println!();
            } else {
                print!(
                    "info depth {} score cp {}, nodes {} nps {} time {} pv ",
                    i + 1,
                    score,
                    self.nodes,
                    nps,
                    time
                );

                for i in 0..self.pv_len[0] {
                    print!("{} ", self.pv[0][i].unwrap());
                }

                println!();
            }
        }

        println!("bestmove {}", bestmove.unwrap());
    }

    fn search(&mut self, depth: usize) -> (Option<Move>, i32) {
        let mut best_move = None;
        let mut best_score = i32::MIN;
        self.nodes = 0;
        self.pv_len.fill(0);

        for mv in self.board.generate_moves() {
            let undo = self.board.make_move(mv);

            let score = -self.negamax(depth - 1, -eval::INF, eval::INF, 1);

            if score > best_score {
                best_score = score;
                best_move  = Some(mv);

                self.pv[0][0] = Some(mv);

                let child_len = self.pv_len[1];

                for i in 0..child_len {
                    self.pv[0][i + 1] = self.pv[1][i];
                }

                self.pv_len[0] = child_len + 1;
            }

            self.board.unmake_move(undo);
        }

        (best_move, best_score)
    }

    fn negamax(&mut self, depth: usize, mut alpha: i32, beta: i32, ply: usize) -> i32 {
        let moves = self.board.generate_moves();

        self.nodes += 1;

        if moves.is_empty() {
            if self.board.is_in_check(self.board.get_turn()) {
                return -eval::MATE + ply as i32;
            } else {
                return eval::REMIS;
            }
        }

        if depth == 0 {
            return match self.board.get_turn() {
                true => eval::evaluate(&self.board),
                false => -eval::evaluate(&self.board),
            };
        }

        let mut max_score = i32::MIN;

        for mv in moves {
            let undo = self.board.make_move(mv);

            let score = -self.negamax(depth - 1, -beta, -alpha, ply + 1);

            if score > max_score {
                max_score = score;

                self.pv[ply][0] = Some(mv);

                let child_len = self.pv_len[ply + 1];

                for i in 0..child_len {
                    self.pv[ply][i + 1] = self.pv[ply + 1][i];
                }

                self.pv_len[ply] = child_len + 1;
            }

            self.board.unmake_move(undo);

            alpha = alpha.max(score);

            if alpha >= beta {
                break;
            }
        }

        max_score
    }
}

impl Config {
    pub fn new(threads: usize) -> Self {
        Self { threads }
    }
}
