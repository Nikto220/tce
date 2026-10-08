#![allow(long_running_const_eval)]

use std::io::{self, BufRead};

pub mod engine;
use engine::*;

pub fn run() {
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
                if tokens.len() > 1 {
                    match tokens[1] {
                        "depth" => {
                            if tokens.len() > 2
                                && let Ok(num) = tokens[2].parse::<usize>()
                            {
                                engine.go_depth(num);
                            }
                        }
                        "perft" => {
                            if tokens.len() > 2
                                && let Ok(num) = tokens[2].parse::<usize>()
                            {
                                engine.perft_divide(num);
                            }
                        }
                        "winc" | "binc" | "wtime" | "btime" => {
                            let (mut wtime, mut btime, mut winc, mut binc) = (0, 0, 0, 0);
                            for i in 1..=4 {
                                if tokens.len() > i * 2 {
                                    match tokens[i * 2 - 1] {
                                        "winc" => {winc = tokens[i * 2].parse::<u64>().unwrap_or(0)},
                                        "binc" => {binc = tokens[i * 2].parse::<u64>().unwrap_or(0)}
                                        "wtime" => {wtime = tokens[i * 2].parse::<u64>().unwrap_or(0)}
                                        "btime" => {btime = tokens[i * 2].parse::<u64>().unwrap_or(0)}
                                        _ => {}
                                    }
                                }
                                else {
                                    break;
                                }
                            }

                            engine.go(wtime, btime, winc, binc);
                        },
                        _ => {}
                    }
                }
            }
            "eval" => {
                println!("info score cp {}", engine.eval());
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
