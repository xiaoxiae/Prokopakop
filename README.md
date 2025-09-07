# <img src="assets/logo.png" alt="Prokopakop Logo" width="48" height="48" align="top"> Prokopakop

A moderately fast UCI chess engine written in Rust that **kops the Prokop**.

**Challenge it on [Lichess](https://lichess.org/@/prokopakop)!**

## Technical Features

### Move Generation

- **[Magic Bitboards](https://www.chessprogramming.org/Magic_Bitboards)** - Fast sliding piece move generation
- **[Generics](https://www.chessprogramming.org/Generic_Programming)** - Const generics for piece/color function variants
- **Iterative [Zobrist Hashing](https://www.chessprogramming.org/Zobrist_Hashing)** - Position hashing for transposition tables

<figure>
    <img src="scripts/benchmark.png" alt="Performance Across Commits" width="100%">
    <figcaption><em>Performance benchmarks across git commits on a i9-14900K. Benchmarks use perft 5, hashing and bulk-counting.</em></figcaption>
</figure>

### Search

- **[Alpha-Beta](https://www.chessprogramming.org/Alpha-Beta)** via **[Negamax](https://www.chessprogramming.org/Negamax)**
- **[Iterative Deepening](https://www.chessprogramming.org/Iterative_Deepening)**
- **[Move Ordering](https://www.chessprogramming.org/Move_Ordering)** - principal variation moves first, then [MVV-LVA](https://www.chessprogramming.org/MVV-LVA)
- **[Quiescence Search](https://www.chessprogramming.org/Quiescence_Search)**
- **[Transposition Table](https://www.chessprogramming.org/Transposition_Table)**

### Evaluation

- [Material](https://www.chessprogramming.org/Material)
- [Piece-Square tables](https://www.chessprogramming.org/Piece-Square_Tables)
- [Pawn Structure](https://www.chessprogramming.org/Pawn_Structure) (doubled pawns, passed pawns)
- [Piece Mobility](https://www.chessprogramming.org/Mobility)


<figure>
    <img src="scripts/tournament.png" alt="Tournament Results Heatmap" width="100%">
    <figcaption><em>Tournament results heatmap showing win rates between different engine versions. Each cell shows the win rate of the row engine against the column engine.</em></figcaption>
</figure>

## Usage

### Build & Run
```bash
cargo run --release
```

### UCI Mode
Prokopakop implements most of the UCI (Universal Chess Interface) protocol for integration with chess GUIs:

```
uci                              # Initialize UCI mode
isready                          # Check engine readiness
ucinewgame                       # Start new game
position startpos                # Set starting position
position startpos moves <moves>  # Set position with moves
position fen <FEN>               # Set position from FEN
setoption name <id> value <val>  # Set engine options
go perft <depth>                 # Run perft test
go <params>                      # Search with various parameters
stop                             # Stop current search
quit                             # Exit engine
```

### Opening Book

Prokopakop supports opening books built from PGN game collections:

#### Building an Opening Book
```bash
# Build opening book from PGN files
prokopakop --build-book games.pgn -o opening.book

# With custom settings
prokopakop --build-book *.pgn -o opening.book --min-count 5 --max-positions 50000
```

**Options:**
- `--min-count N`: Only keep positions that appear at least N times (default: 3)
- `--max-positions N`: Keep at most N positions (default: 100,000)

#### Using an Opening Book
Set the `OwnBook` UCI option to the path of your opening book file:

```
setoption name OwnBook value path/to/opening.book
```

The engine will automatically use opening book moves when available, displaying:
```
info string Using opening book move
```

### Command Line Options
```bash
prokopakop                    # Start in UCI mode
prokopakop --magic            # Bootstrap magic bitboards
prokopakop --build-book <pgn> -o <book>  # Build opening book
```

## Resources

- https://www.chessprogramming.org/Main_Page
- https://ameye.dev/notes/chess-engine/
- https://peterellisjones.com/posts/generating-legal-chess-moves-efficiently/
