use super::super::board::Board;

pub fn evaluate(board: &Board) -> i32 {
    let pieces = board.get_board();

    let white = pieces[0].count_ones() as i32 * 100
        + pieces[1].count_ones() as i32 * 300
        + pieces[2].count_ones() as i32 * 300
        + pieces[3].count_ones() as i32 * 500
        + pieces[4].count_ones() as i32 * 900;

    let black = pieces[6].count_ones() as i32 * 100
        + pieces[7].count_ones() as i32 * 300
        + pieces[8].count_ones() as i32 * 300
        + pieces[9].count_ones() as i32 * 500
        + pieces[10].count_ones() as i32 * 900;

    white - black
}

pub const MATE: i32 = INF;
pub const REMIS: i32 = 0;
pub const INF: i32 = 1_000_000;
