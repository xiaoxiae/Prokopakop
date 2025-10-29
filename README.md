# <img src="assets/logo.png" alt="Prokopakop Logo" width="48" height="48" align="top"> Prokopakop

A moderately fast UCI chess engine written in Rust that **kops the Prokop**.

**Challenge it on [Lichess](https://lichess.org/@/prokopakop)!**

## Bot

### Move Generation

- **[Magic Bitboards](https://www.chessprogramming.org/Magic_Bitboards)** - Fast sliding piece move generation
- **[Generics](https://www.chessprogramming.org/Generic_Programming)** - Const generics for piece/color function variants
- **Iterative [Zobrist Hashing](https://www.chessprogramming.org/Zobrist_Hashing)** - Position hashing for transposition tables

### Search

- **[Alpha-Beta](https://www.chessprogramming.org/Alpha-Beta)** via **[Negamax](https://www.chessprogramming.org/Negamax)** with **[PV-Search](https://www.chessprogramming.org/Principal_Variation_Search)**
- **[Iterative Deepening](https://www.chessprogramming.org/Iterative_Deepening)** with [Aspiration Windows](https://www.chessprogramming.org/Aspiration_Windows)
- **[Move Ordering](https://www.chessprogramming.org/Move_Ordering)** - PV moves, hash moves, [MVV-LVA](https://www.chessprogramming.org/MVV-LVA), [SEE](https://www.chessprogramming.org/Static_Exchange_Evaluation), [killer moves](https://www.chessprogramming.org/Killer_Heuristic), [history heuristic](https://www.chessprogramming.org/History_Heuristic)
- **[Quiescence Search](https://www.chessprogramming.org/Quiescence_Search)** with [Delta Pruning](https://www.chessprogramming.org/Delta_Pruning)
- **[Transposition Table](https://www.chessprogramming.org/Transposition_Table)**
- **[Null Move Pruning](https://www.chessprogramming.org/Null_Move_Pruning)**
- **[Late Move Reduction](https://www.chessprogramming.org/Late_Move_Reductions)**
- **[Futility Pruning](https://www.chessprogramming.org/Futility_Pruning)** (forward + reverse)
- **[Razoring](https://www.chessprogramming.org/Razoring)**
- **[SEE Pruning](https://www.chessprogramming.org/Static_Exchange_Evaluation)**
- **[Quiet Move Pruning](https://www.chessprogramming.org/Futility_Pruning#Quiet_Move_Pruning)**

### Evaluation

Uses a self-trained **NNUE** (transitioned at commit `b0cf0c1`); initially trained on a hand-crafted eval that used

- [Material](https://www.chessprogramming.org/Material) with [Bishop Pair](https://www.chessprogramming.org/Bishop_Pair) bonus
- [Piece-Square tables](https://www.chessprogramming.org/Piece-Square_Tables) with game phase interpolation
- [Pawn Structure](https://www.chessprogramming.org/Pawn_Structure) (doubled pawns, passed pawns, isolated pawns)
- [Piece Mobility](https://www.chessprogramming.org/Mobility) using pseudo-legal move generation
- [King Safety](https://www.chessprogramming.org/King_Safety) (pawn shield, open files, enemy piece attacks in king zone)

## Usage

Run

```bash
cargo run --release
```

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

and also some special commands that are not UCI-compliant, but I'm a rebel:

```
eval                             # Show position evaluation
joke                             # Tells a random joke... just be careful to not ask for too many
```

### Command Line Options

```
UCI Chess Engine, made to kop the Prokop

Usage: prokopakop [OPTIONS]

Options:
      --magic                  Generate magic bitboards
      --training               Generate NNUE training data through self-play
  -g, --games <NUM>            Number of games to play (default: 32) [default: 32]
  -d, --depth <DEPTH>          Fixed search depth per move (default: 8) [default: 8]
  -o, --output <FILE>          Output file for training data (default: data/selfplay.txt) [default: data/selfplay.txt]
      --start-moves-min <NUM>  Minimum number of random starting moves (default: 1) [default: 1]
      --start-moves-max <NUM>  Maximum number of random starting moves (default: 6) [default: 6]
  -h, --help                   Print help
  -V, --version                Print version
```

## NNUE Training

The `train/` directory contains an NNUE neural network trainer (using [bullet](https://github.com/jw1912/bullet)) for improving the engine's evaluation function.

```
Prokopakop's NNUE trainer + utilities

Usage: train <COMMAND>

Commands:
  train        Run hyperparameter optimization training
  deduplicate  Deduplicate FEN positions from a file
  convert      Convert text FEN to binary format
  help         Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

### Training Workflow

1. **Generate Training Data**: Use `prokopakop --training` to play self-play games and generate positions with game outcomes
2. **Process Data**: Use the trainer utility to `convert` data to binary format (see `--help`)
3. **Train the Network**: Create an `experiment-<NAME>` folder, copy `train/example-config.toml` to `experiment-<NAME>/config.toml`, add data created in steps 1-2 to `experiment-<NAME>/data.bin`, and run `train <experiment folder name>` to train


## Resources

- https://www.chessprogramming.org/Main_Page
- https://ameye.dev/notes/chess-engine/
- https://peterellisjones.com/posts/generating-legal-chess-moves-efficiently/
