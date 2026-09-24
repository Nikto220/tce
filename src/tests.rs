use crate::engine::attacks::{BISHOP_DIRS, ROOK_DIRS};

use super::*;

#[test]
fn fentest() {
    let mut board = engine::board::Board::new();
    board.position_fen(&[
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR",
        "w",
        "KQkq",
        "-",
        "0",
        "1",
    ]);
    assert_eq!(
        board.get_board(),
        [
            0x0000_0000_0000_FF00u64, // WP
            0x0000_0000_0000_0042u64, // WN
            0x0000_0000_0000_0024u64, // WB
            0x0000_0000_0000_0081u64, // WR
            0x0000_0000_0000_0008u64, // WQ
            0x0000_0000_0000_0010u64, // WK
            0x00FF_0000_0000_0000u64, // BP
            0x4200_0000_0000_0000u64, // BN
            0x2400_0000_0000_0000u64, // BB
            0x8100_0000_0000_0000u64, // BR
            0x0800_0000_0000_0000u64, // BQ
            0x1000_0000_0000_0000u64, // BK
        ]
    );
}

#[test]
fn make_unmake_test() {
    let mut original = engine::board::Board::new();
    original.position_startpos();
    let mut board = original.clone();

    let undo = board.make_move(board.parse_move("e2e4").unwrap());
    board.unmake_move(undo);

    assert_eq!(original, board);
}

#[test]
fn make_unmake_test_en_passant() {
    let mut original = engine::board::Board::new();
    original.position_startpos_moves(&["e2e4", "c7c5", "e4e5", "f7f5"]);
    let mut board = original.clone();

    let undo = board.make_move(board.parse_move("e5f6").unwrap());
    board.unmake_move(undo);

    assert_eq!(original, board);
}

#[test]
fn make_unmake_test_capture() {
    let mut original = engine::board::Board::new();
    original.position_startpos_moves(&["e2e4", "f7f5"]);
    let mut board = original.clone();

    let undo = board.make_move(board.parse_move("e4f5").unwrap());
    board.unmake_move(undo);

    assert_eq!(original, board);
}

#[test]
fn make_unmake_test_quiet() {
    let mut original = engine::board::Board::new();
    original.position_startpos_moves(&["e2e4", "c7c5"]);
    let mut board = original.clone();

    let undo = board.make_move(board.parse_move("e4e5").unwrap());
    board.unmake_move(undo);

    assert_eq!(original, board);
}

#[test]
fn make_unmake_test_castle_king() {
    let mut original = engine::board::Board::new();
    original.position_fen(&[
        "rnbqkbnr/pppp2pp/5p2/4p3/2B5/4PN2/PPPP1PPP/RNBQK2R",
        "w",
        "KQkq",
        "-",
        "0",
        "1",
    ]);
    let mut board = original.clone();

    let undo = board.make_move(board.parse_move("e1g1").unwrap());
    board.unmake_move(undo);

    assert_eq!(original, board);
}

#[test]
fn make_unmake_test_castle_queen() {
    let mut original = engine::board::Board::new();
    original.position_fen(&[
        "rnbqkbnr/pp2pppp/2p5/8/Q1P5/B1N5/P2PPPPP/R3KBNR",
        "w",
        "KQkq",
        "-",
        "0",
        "1",
    ]);
    let mut board = original.clone();

    let undo = board.make_move(board.parse_move("e1c1").unwrap());
    board.unmake_move(undo);

    assert_eq!(original, board);
}

#[test]
fn make_unmake_test_promotion() {
    let mut original = engine::board::Board::new();
    original.position_fen(&["7k/P7/1K6/8/8/8/8/8", "w", "-", "-", "0", "1"]);
    let mut board = original.clone();

    let undo = board.make_move(board.parse_move("a7a8q").unwrap());
    board.unmake_move(undo);

    assert_eq!(original, board);
}

#[test]
fn make_unmake_test_promotion_capture() {
    let mut original = engine::board::Board::new();
    original.position_fen(&["1p5k/P7/1K6/8/8/8/8/8", "w", "-", "-", "0", "1"]);
    let mut board = original.clone();

    let undo = board.make_move(board.parse_move("a7b8q").unwrap());
    board.unmake_move(undo);

    assert_eq!(original, board);
}

#[test]
fn make_unmake_sequence() {
    let mut original = engine::board::Board::new();
    original.position_startpos();

    let mut board = original.clone();
    let mut undos = Vec::new();

    for move_str in ["e2e4", "e7e5", "g1f3", "b8c6", "f1b5"] {
        let mv = board.parse_move(move_str).unwrap();
        undos.push(board.make_move(mv));
    }

    while let Some(undo) = undos.pop() {
        board.unmake_move(undo);
    }

    assert_eq!(original, board);
}

#[test]
fn bishop_attacks_test() {
    println!("{:b}", engine::attacks::slow_attacks(27u8, 0u64, &BISHOP_DIRS));
    assert_eq!(
        engine::attacks::slow_attacks(27u8, 0u64, &BISHOP_DIRS),
        0b10000000_01000001_00100010_00010100_00000000_00010100_00100010_01000001u64
    );
}

#[test]
fn rook_attacks_test() {
    println!("{:b}", engine::attacks::slow_attacks(27u8, 0u64, &ROOK_DIRS));
    assert_eq!(
        engine::attacks::slow_attacks(27u8, 0u64, &ROOK_DIRS),
        0b00001000_00001000_00001000_00001000_11110111_00001000_00001000_00001000u64
    );
}

#[test]
fn rook_attacks_opponents_test() {
    println!("{:b}", engine::attacks::slow_attacks(27u8, 0u64, &ROOK_DIRS));
    assert_eq!(
        engine::attacks::slow_attacks(27u8, 0b00001000_00000000u64, &ROOK_DIRS),
        0b00001000_00001000_00001000_00001000_11110111_00001000_00001000_00000000u64
    );
}

#[test]
fn bishop_attacks_opponents_test() {
    println!("{:b}", engine::attacks::slow_attacks(27u8, 0u64, &BISHOP_DIRS));
    assert_eq!(
        engine::attacks::slow_attacks(27u8, 0b00010000_00000000_00000000u64, &BISHOP_DIRS),
        0b10000000_01000001_00100010_00010100_00000000_00010100_00000010_00000001u64
    );
}
