use bitvec::{prelude::*, slice::IterOnes};
use serde::ser::{Serialize, Serializer, SerializeStruct};
use serde::de::{Deserialize, Deserializer, MapAccess, Visitor};
use lazy_static::lazy_static;

const X: usize = 3;
const Y: usize = 8;
const B: usize = X * Y;
pub type BitBoard = BitArr!(for B, in u8, Lsb0);
// every 8 values represents one ring, outermost to innermost
// each ring starts from top left corner and goes in clockwise order

pub trait BitArr2D {
    fn empty() -> Self;
    fn set_point(&mut self, x: usize, y: usize, value: bool);
    fn from_point(x: usize, y: usize) -> Self;
    type IterPoints<'a>: Iterator<Item=(usize, usize)> + 'a where Self: 'a;
    fn iter_set_points(&'_ self) -> Self::IterPoints<'_>;
}

impl BitArr2D for BitBoard {
    fn empty() -> Self {
        bitarr!(u8, Lsb0; 0; B)
    }

    fn set_point(&mut self, x: usize, y: usize, value: bool) {
        let idx = (x % X) * Y + (y % Y);
        self.set(idx as usize, value);
    }

    fn from_point(x: usize, y: usize) -> Self {
        let mut square: BitBoard = BitBoard::empty();
        square.set_point(x, y, true);
        square
    }

    type IterPoints<'a> = std::iter::Map<IterOnes<'a, u8, Lsb0>, fn(usize) -> (usize, usize)>;

    fn iter_set_points(&'_ self) -> Self::IterPoints<'_> {
        self.iter_ones().map(|idx| (idx / Y, idx % Y))
    }
}

const MIN_PIECES: u8 = 3;
const MAX_PIECES: u8 = 9;
lazy_static! {
    static ref MILL_MASKS: Vec<BitBoard> = {
        let mut mill_masks = Vec::<BitBoard>::new();
        for x in 0..X {
            for y in 0..Y {
                if y % 2 == 0 {  // corner square
                    let mut mill_mask = BitBoard::empty();
                    for i in 0..3 {
                        mill_mask.set_point(x, y + i, true);
                    }
                    mill_masks.push(mill_mask);
                } else if x == 0 {  // edge square, outermost ring
                    let mut mill_mask = BitBoard::empty();
                    for i in 0..3 {
                        mill_mask.set_point(x + i, y, true);
                    }
                    mill_masks.push(mill_mask);
                }
            }
        }
        mill_masks
    };
}

#[derive(PartialEq, Eq, Clone)]
pub struct Move {
    square: BitBoard,
    from_square: Option<BitBoard>,
    remove_square: Option<BitBoard>,
}

impl Serialize for Move {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let square = self.square.iter_set_points().next();
        let from_square = match self.from_square {
            Some(sq) => sq.iter_set_points().next(),
            None => None
        };
        let remove_square = match self.remove_square {
            Some(sq) => sq.iter_set_points().next(),
            None => None
        };

        let mut s = serializer.serialize_struct("Move", 3)?;
        let _ = s.serialize_field("square", &square);
        let _ = s.serialize_field("from_square", &from_square);
        let _ = s.serialize_field("remove_square", &remove_square);
        s.end()
    }
}

struct MoveVisitor;
impl<'de> Visitor<'de> for MoveVisitor {
    type Value = Move;
    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a JSON object for Move")
    }
    fn visit_map<V>(self, mut map: V) -> Result<Move, V::Error> where V: MapAccess<'de> {
        let mut square = None;
        let mut from_square = None;
        let mut remove_square = None;
        while let Some(key) = map.next_key::<String>()? {
            let field = match key.as_str() {
                "square" => &mut square,
                "from_square" => &mut from_square,
                "remove_square" => &mut remove_square,
                _ => { return Err(serde::de::Error::unknown_field(&key, &["square", "from_square", "remove_square"])); }
            };
            let opt_arr = map.next_value::<Option<Vec<usize>>>()?;
            if let Some(arr) = opt_arr {
                *field = Some(BitBoard::from_point(arr[0], arr[1]));
            }
        }
        let square = square.ok_or_else(|| serde::de::Error::missing_field("square"))?;
        Ok(Move { square, from_square, remove_square })
    }
}

impl<'de> Deserialize<'de> for Move {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        deserializer.deserialize_map(MoveVisitor)
    }
}

#[derive(PartialEq, Eq, Clone)]
pub struct Board {
    black_squares: BitBoard,
    white_squares: BitBoard,
    initial_placed: u8,
    num_black_pieces: u8,
    num_white_pieces: u8,
    pub blacks_turn: bool,
}

impl Board {
    pub fn new() -> Self {
        Self {
            black_squares: BitBoard::empty(),
            white_squares: BitBoard::empty(),
            initial_placed: 0,
            num_black_pieces: 0,
            num_white_pieces: 0,
            blacks_turn: true,
        }
    }

    fn empty_squares(&self) -> BitBoard {
        !(self.black_squares | self.white_squares)
    }

    pub fn is_game_over(&self) -> bool {
        self.is_black_winner() || self.is_white_winner()
    }

    pub fn is_black_winner(&self) -> bool {
        self.is_movement_phase() && self.num_white_pieces < MIN_PIECES
    }

    pub fn is_white_winner(&self) -> bool {
        self.is_movement_phase() && self.num_black_pieces < MIN_PIECES
    }

    fn is_placement_phase(&self) -> bool {
        self.initial_placed <= MAX_PIECES * 2
    }

    fn is_movement_phase(&self) -> bool {
        !self.is_placement_phase()
    }

    fn is_flying_phase(&self) -> bool {
        self.is_movement_phase() && (
            if self.blacks_turn { self.num_black_pieces == MIN_PIECES }
            else { self.num_white_pieces == MIN_PIECES }
        )
    }

    fn get_player_squares(&self) -> (BitBoard, BitBoard) {
        if self.blacks_turn { (self.black_squares, self.white_squares) } else { (self.white_squares, self.black_squares) }
    }

    fn adjacent_squares(x: usize, y: usize) -> BitBoard {
        let mut adj_mask: BitBoard = BitBoard::empty();
        adj_mask.set_point(x, y.wrapping_sub(1), true);
        adj_mask.set_point(x, y + 1, true);
        if y % 2 == 1 { // edge square
            if x > 0 {
                adj_mask.set_point(x.wrapping_sub(1), y, true);
            }
            if x < X.wrapping_sub(1) {
                adj_mask.set_point(x + 1, y, true);
            }
        }
        adj_mask
    }

    fn mill_squares(piece_squares: BitBoard) -> BitBoard {
        let mut result = BitBoard::empty();
        for mill_mask in (&*MILL_MASKS).into_iter() {
            if *mill_mask & piece_squares == *mill_mask {
                result |= *mill_mask
            }
        }
        result
    }

    fn creates_mill(piece_squares: BitBoard, square: BitBoard) -> bool {
        Self::mill_squares(piece_squares | square) > Self::mill_squares(piece_squares)
    }

    fn generate_moves(
            target_squares: BitBoard,
            from_square: Option<BitBoard>,
            my_squares: BitBoard,
            opp_removable_squares: BitBoard,
    ) -> Vec<Move> {
        target_squares.iter_set_points().flat_map(move |(x, y)| -> Vec<Move> {
            let square = BitBoard::from_point(x, y);
            if Self::creates_mill(my_squares, square) {
                opp_removable_squares.iter_set_points().map(move |(rx, ry)| Move {
                    square: square.clone(),
                    from_square: from_square.clone(),
                    remove_square: Some(BitBoard::from_point(rx, ry)),
                }).collect()
            } else {
                std::iter::once(Move {
                    square: square.clone(),
                    from_square: from_square.clone(),
                    remove_square: None,
                }).collect()
            }
        }).collect()
    }

    pub fn legal_moves(&self) -> Vec<Move> {
        if self.is_game_over() {
            return Vec::<Move>::new();
        }

        let empty_squares = self.empty_squares();
        let (my_squares, opp_squares) = self.get_player_squares();
        let opp_removable_squares = opp_squares & !Self::mill_squares(opp_squares);

        if self.is_placement_phase() {
            Self::generate_moves(empty_squares, None, my_squares, opp_removable_squares)
        } else if self.is_flying_phase() {
            my_squares.iter_set_points().flat_map(|(x, y)| {
                let from_square = Some(BitBoard::from_point(x, y));
                Self::generate_moves(empty_squares, from_square, my_squares, opp_removable_squares)
            }).collect()
        } else {
            my_squares.iter_set_points().flat_map(|(x, y)| {
                let from_square = Some(BitBoard::from_point(x, y));
                let adjacent_empty_squares = Self::adjacent_squares(x, y) & empty_squares;
                Self::generate_moves(adjacent_empty_squares, from_square, my_squares, opp_removable_squares)
            }).collect()
        }
    }

    pub fn make_move(&self, maybe_move: Option<Move>) -> Result<Self, std::io::Error> {
        let mut board = self.clone();
        let (my_squares, opp_squares, num_my_pieces, num_opp_pieces) = if self.blacks_turn {
            (&mut board.black_squares, &mut board.white_squares, &mut board.num_black_pieces, &mut board.num_white_pieces)
        } else {
            (&mut board.white_squares, &mut board.black_squares, &mut board.num_white_pieces, &mut board.num_black_pieces)
        };
        let initial_placed = &mut board.initial_placed;
        let legal_moves = self.legal_moves();

        match maybe_move {
            Some(move_) if legal_moves.contains(&move_) => {
                // update squares on copy of board variables
                *my_squares |= move_.square;
                if let Some(from_square) = move_.from_square {
                    *my_squares &= !from_square;
                } else {
                    *num_my_pieces += 1;
                    *initial_placed += 1;
                }
                if let Some(remove_square) = move_.remove_square {
                    *opp_squares &= !remove_square;
                    *num_opp_pieces -= 1;
                }
            },
            None if legal_moves.is_empty() => {}, // pass if no legal moves and None is provided
            _ => { return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Illegal move")); }, // otherwise illegal move
        }

        // return new board state
        board.blacks_turn = !self.blacks_turn;
        Ok(board)
    }
}
