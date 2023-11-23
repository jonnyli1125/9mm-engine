use crate::board::{Board, Move};
use std::collections::HashMap;

pub struct Engine {
    cache: HashMap<u64, f64>,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new()
        }
    }

    pub fn eval(&self, board: &Board) -> f64 {
        // TODO heuristic evaluations of current state
        0f64
    }

    pub fn best_move(&self, board: &Board, max_ms: u32, max_depth: u32) -> Option<Move> {
        // TODO alpha beta minimax search
        // in-place
        // transposition table
        // iterative deepening
        // pondering
        // TODO check if new state has legal moves. if not, use a dummy step in the minimax search.
        None
    }
}
