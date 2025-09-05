# Prokopakop

A moderately fast UCI chess engine written in Rust that **kops the Prokop**.

<figure>
    <img src="scripts/plots/nps_plot.png" alt="Performance Across Commits" width="100%">
    <figcaption><em>Performance benchmarks across git commits on a i9-14900K. Benchmarks use perft 5, hashing and bulk-counting.</em></figcaption>
</figure>

## Technical Features

### Move Generation

- **[Magic Bitboards](https://www.chessprogramming.org/Magic_Bitboards)** - Fast sliding piece move generation
- **[Generics](https://www.chessprogramming.org/Generic_Programming)** - Const generics for piece/color function variants
- **Iterative [Zobrist Hashing](https://www.chessprogramming.org/Zobrist_Hashing)** - Position hashing for transposition tables

### Search + Evaluation

- **[Alpha-Beta](https://www.chessprogramming.org/Alpha-Beta) Search** via **[Negamax](https://www.chessprogramming.org/Negamax)** - Negamax search with alpha-beta pruning
- **[Iterative Deepening](https://www.chessprogramming.org/Iterative_Deepening)** - Progressive search depth

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

### Command Line Options
```bash
prokopakop                    # Start in UCI mode
prokopakop --magic            # Bootstrap magic bitboards
```

## Resources

- https://www.chessprogramming.org/Main_Page
- https://ameye.dev/notes/chess-engine/
- https://peterellisjones.com/posts/generating-legal-chess-moves-efficiently/
