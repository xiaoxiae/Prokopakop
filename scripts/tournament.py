#!/usr/bin/env python3

import argparse
import subprocess
import sys
import json
import numpy as np
import matplotlib.pyplot as plt
import seaborn as sns

from pathlib import Path
import shlex

COMMIT_NAMES = [
    # "6aec863",  # functional alpha/beta + iterative deepening + material eval with position tables
    # "53a09e5",  # move ordering via pv + mvv-lva
    # "bbef3be",  # quiescence search
    # "35f6fb6",  # transposition table
    # "ff14c29",  # faster eval
    # "7ff40fc",  # threefold repetition detection
    # "ab3fdc8",  # passed / doubled pawns
    # "6c4e7ee",  # piece mobility
    # "a28d291",  # delta pruning for quiescence search
    # "2c1a839",  # killer moves
    # "fed61d5",  # fast passed pawn eval
    # "10c64e9",  # null move pruning
    # "5047857",  # late move reduction
    # "93743a2",  # bishop pair + faster move generation
    # "a56e0f7",  # better piece tables + isolated pawns
    # "1fb64eb",  # LMR bugfix
    # "7cfbb74",  # PV search
    # "5ddad41",  # bucket TT
    # "6e3bad1",  # history heuristic
    # "c14002b",  # pseudo-legal move generation
    # "e6662f0",  # LMR bug
    "075f411",  # actually, no pseudo-legal move generation
    # "7490eba",  # futility pruning
    "8024a6e",  # no partial result usage + better iterative deepening time management
    # "c144fb9",  # no TT cleaning between moves
    "5cc228e",  # king safety
    "910fb21",  # DP + NMP tuning
]

MASTER_OPTIONS = {
}


def find_binary(version_name):
    """Find the binary for a given version name."""
    # Extract the suffix after 'prokop-' if version_name starts with 'prokop-'
    binary_prefix = f"prokopakop-{version_name}"
    release_dir = Path("target") / "release"

    for binary_path in release_dir.iterdir():
        if binary_path.is_file() and binary_path.name.startswith(binary_prefix):
            return binary_path

    return None


def find_master_binary():
    """Find the main prokopakop binary."""
    release_dir = Path("target") / "release"
    master_binary = release_dir / "prokopakop"

    if master_binary.exists() and master_binary.is_file():
        return master_binary

    return None


def get_commit_info(commit_item):
    """Extract commit name and options from commit item (supports both string and dict formats)."""
    if isinstance(commit_item, str):
        return commit_item, {}
    elif isinstance(commit_item, dict):
        return commit_item["commit"], commit_item.get("options", {})
    else:
        raise ValueError(f"Invalid commit format: {commit_item}")


def add_uci_options(cmd, options):
    """Add UCI options to the command list."""
    for option_name, option_value in options.items():
        cmd.extend([f"option.{option_name}={option_value}"])


def build_fastchess_command(commit_names, add_master=False, last_n=None):
    """
    Build the fastchess command with multiple engines based on commit names.

    Args:
        commit_names: List of commit names to include
        add_master: Whether to include the master binary
        last_n: If specified, only include the last n commits (most recent)
    """
    # Base command
    cmd = ["./bin/fastchess/fastchess"]

    # Determine which commits to use
    commits_to_use = commit_names
    if last_n is not None:
        # Take the last n commits (most recent are at the end of the list)
        commits_to_use = commit_names[-last_n:] if last_n <= len(commit_names) else commit_names

    # Add master binary if requested
    if add_master:
        master_binary = find_master_binary()
        if master_binary is None:
            print("Error: Could not find master binary at target/release/prokopakop")
            sys.exit(1)

        cmd.extend([
            "-engine",
            f"cmd={master_binary}",
            "name=master"
        ])
        # Add UCI options for master
        add_uci_options(cmd, MASTER_OPTIONS)

    # Add engine parameters for each commit
    for i, commit_item in enumerate(commits_to_use):
        commit_name, commit_options = get_commit_info(commit_item)

        binary_path = find_binary(commit_name)
        if binary_path is None:
            print(f"Error: Could not find binary for commit {commit_name}")
            sys.exit(1)

        # Calculate the original index for naming consistency
        original_index = commit_names.index(commit_item) if commit_item in commit_names else i
        engine_name = commit_name

        # Add -{original_index} so we can know which ones are the most recent
        cmd.extend([
            "-engine",
            f"cmd={binary_path}",
            f"name={engine_name}-{original_index}"
        ])
        # Add UCI options for this commit
        add_uci_options(cmd, commit_options)

    # Add common parameters
    cmd.extend([
        "-each", "tc=10+0.1", "restart=on",
        "-rounds", "200",
        "-concurrency", "32",
        "-config", "outname=scripts/tournament_results.json",
        "-openings", "file=data/book.pgn", "format=pgn", "plies=8", "order=random",
    ])

    # Wrap command with grep filter to remove noisy lines
    cmd_str = " ".join(shlex.quote(arg) for arg in cmd)
    filtered_cmd = f"{cmd_str} | grep -Ev '^(Moves|Info|Warning|Position);'"

    return ["bash", "-c", filtered_cmd]


def run_fastchess(commit_names, add_master=False, last_n=None):
    """
    Run fastchess with the specified commit engines.

    Args:
        commit_names: List of commit names to include
        add_master: Whether to include the master binary
        last_n: If specified, only include the last n commits (most recent)
    """
    command = build_fastchess_command(commit_names, add_master, last_n)

    print("Running fastchess with the following command:")
    print(" ".join(command))
    print()

    try:
        # Run the command
        result = subprocess.run(command, check=True, text=True)
        print("Fastchess completed successfully!")
        return result.returncode
    except subprocess.CalledProcessError as e:
        print(f"Error running fastchess: {e}")
        return e.returncode
    except FileNotFoundError:
        print("Error: fastchess binary not found at ./bin/fastchess/fastchess")
        print("Please ensure the binary exists and is executable.")
        return 1


def calculate_win_rate(stats, engine1, engine2):
    """Calculate win rate of engine1 against engine2."""
    key = f"{engine1} vs {engine2}"
    if key in stats:
        data = stats[key]
        total_games = data['wins'] + data['losses'] + data['draws']
        if total_games == 0:
            return 0.5
        # Win rate = (wins + 0.5 * draws) / total_games
        win_rate = (data['wins'] + 0.5 * data['draws']) / total_games
        return win_rate
    return None


def generate_heatmap(results_file="scripts/tournament_results.json", add_master=False):
    """Generate a heatmap from tournament results."""

    # Load the tournament results
    try:
        with open(results_file, 'r') as f:
            data = json.load(f)
    except FileNotFoundError:
        print(f"Results file {results_file} not found. Run the tournament first.")
        return

    stats = data.get('stats', {})

    # Extract unique engine names and sort them
    engines = set()
    for matchup in stats.keys():
        parts = matchup.split(' vs ')
        if len(parts) == 2:
            engines.add(parts[0])
            engines.add(parts[1])

    # Sort engines by their index number (newer engines have lower indices)
    engine_list = sorted(list(engines), key=lambda x: int(x.split('-')[-1]) if x != 'master' else -1)

    # Create a matrix for the heatmap
    n = len(engine_list)
    win_matrix = np.full((n, n), np.nan)

    # Fill the matrix with win rates
    for i, engine1 in enumerate(engine_list):
        for j, engine2 in enumerate(engine_list):
            if i == j:
                win_matrix[len(engine_list) - i - 1, j] = 0.5  # 50% against itself
            else:
                win_rate = calculate_win_rate(stats, engine1, engine2)
                if win_rate is not None:
                    win_matrix[len(engine_list) - i - 1, j] = win_rate

    # Create the heatmap
    plt.figure(figsize=(12, 4))

    # Create custom colormap (red for losses, white for 50%, green for wins)
    cmap = sns.diverging_palette(10, 130, as_cmap=True)

    # Create heatmap with annotations
    ax = sns.heatmap(win_matrix,
                     annot=True,
                     fmt='.1%',
                     cmap=cmap,
                     center=0.5,
                     vmin=0,
                     vmax=1,
                     cbar_kws={'label': 'Win Rate'},
                     xticklabels=engine_list,
                     yticklabels=list(reversed(engine_list)))

    # Improve labels
    plt.title('Version Performance Heatmap', fontsize=16, pad=20)

    # Add grid for better readability
    ax.set_facecolor('white')
    for i in range(n + 1):
        ax.axhline(i, color='gray', linewidth=0.5)
        ax.axvline(i, color='gray', linewidth=0.5)

    plt.tight_layout()

    # Save the heatmap
    output_file = "scripts/tournament.png"
    plt.savefig(output_file, dpi=150, bbox_inches='tight')
    print(f"\nHeatmap saved to {output_file}")


def print_engine_info(commit_names, add_master=False, last_n=None):
    """Print information about engines and their options."""
    commits_to_use = commit_names
    if last_n is not None:
        commits_to_use = commit_names[-last_n:] if last_n <= len(commit_names) else commit_names

    total_engines = len(commits_to_use)
    if add_master:
        total_engines += 1

    print(f"Engines configured ({total_engines} total):")

    if add_master:
        options_str = ""
        if MASTER_OPTIONS:
            options_list = [f"{k}={v}" for k, v in MASTER_OPTIONS.items()]
            options_str = f" (options: {', '.join(options_list)})"
        print(f"  master (target/release/prokopakop){options_str}")

    for i, commit_item in enumerate(commits_to_use):
        commit_name, commit_options = get_commit_info(commit_item)
        original_index = commit_names.index(commit_item) if commit_item in commit_names else i
        options_str = ""
        if commit_options:
            options_list = [f"{k}={v}" for k, v in commit_options.items()]
            options_str = f" (options: {', '.join(options_list)})"
        print(f"  {original_index}. {commit_name} (target/release/prokopakop-{commit_name}){options_str}")


def main():
    """
    Main function to run the fastchess tournament.
    """
    parser = argparse.ArgumentParser(description="Run fastchess tournament with specified engines")
    parser.add_argument("--add-master", action="store_true",
                       help="Include the current prokopakop binary (master) in the tournament")
    parser.add_argument("--heatmap-only", action="store_true",
                       help="Only generate the heatmap from existing results without running tournament")
    parser.add_argument("-n", "--last-n", type=int, metavar="N",
                       help="Only include the last N commits (most recent) in the tournament")
    parser.add_argument("--results-file", default="scripts/tournament_results.json",
                       help="Path to the tournament results JSON file")

    args = parser.parse_args()

    commits_to_use = COMMIT_NAMES

    # Validate last_n parameter
    if args.last_n is not None:
        if args.last_n <= 0:
            print("Error: -n must be a positive integer")
            sys.exit(1)
        if args.last_n > len(COMMIT_NAMES):
            print(f"Warning: Requested {args.last_n} commits, but only {len(COMMIT_NAMES)} available. Using all commits.")

    if args.heatmap_only:
        # Just generate the heatmap from existing results
        print("Generating heatmap from existing results...")
        generate_heatmap(args.results_file, args.add_master)
    else:
        # Run the tournament
        if args.last_n is not None:
            tournament_type = f"tournament (last {args.last_n} commits)"
            if args.last_n == 2:
                tournament_type = "duel (last 2 commits)"
        else:
            tournament_type = "full tournament"

        print(f"Setting up fastchess {tournament_type}:")
        print_engine_info(commits_to_use, args.add_master, args.last_n)
        print()

        # Run fastchess
        exit_code = run_fastchess(commits_to_use, args.add_master, args.last_n)

        if exit_code == 0:
            # Generate heatmap after successful tournament
            print(f"\nGenerating performance heatmap for {tournament_type}...")
            generate_heatmap(args.results_file, args.add_master)

        sys.exit(exit_code)


if __name__ == "__main__":
    main()
