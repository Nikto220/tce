#![allow(long_running_const_eval)]

use std::io::{self, BufRead};

pub mod engine;
use engine::*;

fn main() {
    let stdin = io::stdin();
    let mut engine = Engine::new(Config::new(1));

    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.is_empty() {
            continue;
        }

        match tokens[0] {
            "uci" => {
                println!("id name TCE");
                println!("id author Nikto");
                println!("uciok");
            }
            "isready" => {
                println!("readyok");
            }
            "ucinewgame" => {
                engine.newgame();
            }
            "position" => {
                if tokens.len() > 1 {
                    match tokens[1] {
                        "startpos" => {
                            if tokens.len() > 2 {
                                if tokens[2] == "moves" {
                                    engine.position_startpos_moves(&tokens[3..]);
                                }
                            } else {
                                engine.position_startpos();
                            }
                        }
                        "fen" => {
                            engine.position_fen(&tokens[2..]);
                        }
                        _ => {}
                    }
                }
            }
            "go" => {
                engine.go();
            }
            "quit" => {
                break;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;
