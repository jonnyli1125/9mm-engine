// local board object
// this will be a simplified version of the server-side board object
var board = {
  blackPieces: [],
  whitePieces: [],
  blacksTurn: true,
  initPlaced: 0,
  legalMoves: [],
  logicalToPhysical: function(square) {
    // transform logical square coordinates to physical square coordinates
    let [x, y] = square;
    let boardRect = document.getElementById('board').getBoundingClientRect();
    let rect = document.querySelector(`.cell.valid[data-x="${x}"][data-y="${y}"]`).getBoundingClientRect();
    let physX = ((rect.left - boardRect.left) + rect.width / 2) / boardRect.width * 100;
    let physY = ((rect.top - boardRect.top) + rect.height / 2) / boardRect.height * 100;
    return [physX, physY]; // return as relative positions (percentages)
  },
  physicalElementToLogical: function(elm) {
    // transform physical element to logical square coordinates
    return [parseInt(elm.dataset.x), parseInt(elm.dataset.y)];
  },
  setPieceCoords: function(elm, square) {
    // set physical coords of an existing piece element given the logical coords
    let [physX, physY] = this.logicalToPhysical(square);
    elm.style.left = physX + '%';
    elm.style.top = physY + '%';
  },
  getPiece: function(square) {
    // get a piece element from the given logical square coords
    let [x, y] = square;
    return document.querySelector(`.piece[data-x="${x}"][data-y="${y}"]`);
  },
  createPiece: function(square) {
    // create a new piece element at the given logical square coords
    let color = this.blacksTurn ? 'black' : 'white';
    let elm = document.createElement('div');
    let [x, y] = square;
    elm.dataset.x = x;
    elm.dataset.y = y;
    this.setPieceCoords(elm, square);
    elm.classList.add('piece', color);
    elm.id = `piece-${this.initPlaced}`;
    elm.addEventListener('click', onPieceClick);
    this.initPlaced += 1;
    document.getElementById('board').appendChild(elm);
    return elm;
  },
  makeMove: function(move) {
    if (move != null) {
      // make move and update physical pieces
      if (move.from_square == null) {
        this.createPiece(move.square);
      } else {
        let piece = this.getPiece(move.from_square);
        this.setPieceCoords(piece, move.square);
      }
      if (move.remove_square != null) {
        this.getPiece(move.remove_square).remove();
      }
    }
    this.blacksTurn = !this.blacksTurn;
  },
  isMoveEqual: function(moveA, moveB) {
    if (moveA == moveB) {
      return true;
    }
    if (moveA == null || typeof moveA != "object" || moveB == null || typeof moveB != "object") {
      return false;
    }
    let keys = ['square', 'from_square', 'remove_square'];
    for (let key of keys) {
      let a = moveA[key];
      let b = moveB[key];
      if (a == b) { continue; }
      if (a == null || b == null) { return false; }
      if (a[0] != b[0] || a[1] != b[1]) { return false; }
    }
    return true;
  },
  isLegalMove: function(move) {
    for (let legalMove of this.legalMoves) {
      if (this.isMoveEqual(move, legalMove)) {
        return true;
      }
    }
    return false;
  }
};

var client = {
  playerIsBlack: true,
  gameStarted: false,
  waitingFor: null,
  selectedPiece: null,
  selectedFromSquare: null,
  websocket: null,
  serverUrl: 'ws://localhost:999',
  displayMessage: function(className) {
    let messages = document.getElementById('messages');
    messages.innerHTML = '';
    let message = document.createElement('p');
    message.classList.add('typed', className);
    messages.appendChild(message);
    let startButtons = document.getElementById('start-buttons');
    if (className == 'start' || className == 'you-won' || className == 'you-lost') {
      startButtons.removeAttribute('style');
    } else {
      startButtons.style.display = 'none';
    }
  },
  startGame: function(playBlack) {
    this.playerIsBlack = playBlack;
    this.gameStarted = true;
    this.waitingFor = playBlack ? 'client-placement' : 'server';
    this.displayMessage('black-placement');
    // start websocket connection and send start message to server
    this.websocket = new WebSocket(this.serverUrl);
    this.websocket.addEventListener('message', onMessage);
    this.websocket.addEventListener('error', (e) => {
      this.displayMessage('no-connection');
      console.log(client);
      console.log(board);
      console.log(e);
    });
    this.websocket.addEventListener('open', () => {
      this.websocket.send(JSON.stringify({ 'start': this.playerIsBlack }));
    });
  },
  resetGame: function() {
    this.playerIsBlack = true;
    this.gameStarted = false;
    this.waitingFor = null;
    this.selectedPiece = null;
  },
  endGame: function(winnerIsBlack) {
    // TODO display modal with win color
    // on modal OK click, resetGame()
    this.displayMessage(winnerIsBlack == this.playerIsBlack ? 'you-won' : 'you-lost');
    this.websocket.close();
    this.websocket = null;
  },
  makeMove: function(move) {
    board.makeMove(move);
    this.selectedFromSquare = null;
    this.selectedPiece = null;
    this.websocket.send(JSON.stringify({ 'move': move }), (e) => {
      onMessage(e);
      // TODO update info message based on available legal moves
    });
    client.waitingFor = 'server';
  }
};

function onMessage(e) {
  // client message protocol: "start", "move"
  // server message protocol: "move", "legal_moves", "error", "end"
  let msg = JSON.parse(e.data);
  console.log(msg);
  if (Object.hasOwn(msg, 'error')) {
    // TODO display error
    return;
  }
  if (Object.hasOwn(msg, 'end')) {
    client.endGame(msg.end);
    return;
  }
  if (Object.hasOwn(msg, 'move')) {
    board.makeMove(msg.move);
  }
  if (Object.hasOwn(msg, 'legal_moves')) {
    board.legalMoves = msg.legal_moves;
    console.log('legal_moves', board.legalMoves);
    // if no legal moves, auto-send a null move back to skip this turn
    if (board.legalMoves.length == 0) {
      setTimeout(() => { client.makeMove(null); }, 1);
    }
  }
}

function onBoardClick(e) {
  let square = board.physicalElementToLogical(e.currentTarget);
  if (square == null) return;
  // event handler for clicking an empty square
  let move = null;
  switch (client.waitingFor) {
    case 'client-placement':
      move = {
        square: square,
        from_square: null,
        remove_square: null
      };
      if (!board.isLegalMove(move)) return;
      break;
    case 'client-movement':
      // TODO check if this move is legal
      if (client.selectedPiece == null) return;
      move = {
        square: square,
        from_square: client.selectedFromSquare,
        remove_square: null
      };
      break;
    default: return;
  }
  // TODO check if any legal removal moves, if so, set client.waitingFor = 'client-removal'
  // else send move to server
  console.log(move);
  if (move != null) {
    client.makeMove(move);
  }
}

function onPieceClick(e) {
  let square = board.physicalElementToLogical(e.currentTarget);
  // event handler for clicking a piece
  switch (client.waitingFor) {
    case 'client-movement':
      client.selectedPiece = e.currentTarget;
      client.selectedFromSquare = square;
      break;
    case 'client-removal':
      // TODO check if this move is legal
      let move = {
        square: board.physicalElementToLogical(client.selectedPiece),
        from_square: client.selectedFromSquare,
        remove_square: square,
      }
      client.makeMove(move);
      break;
  }
}

document.addEventListener('DOMContentLoaded', () => {
  document.getElementById('play-black').addEventListener('click', (e) => { client.startGame(true); });
  document.getElementById('play-white').addEventListener('click', (e) => { client.startGame(false); });
  document.querySelectorAll('.cell.valid').forEach((el) => {
    el.addEventListener('click', onBoardClick);
  });
});
