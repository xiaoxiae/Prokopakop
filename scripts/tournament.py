#!/usr/bin/env python3

import argparse
import subprocess
import sys
import json
import numpy as np
import matplotlib.pyplot as plt
import seaborn as sns

from pathlib import Path

# List of commit names - modify this list as needed
# Lower values should correspond to newer commits
COMMIT_NAMES = [
    # these three were essentially random due to critical bugs in search/eval
    # "9fec3a0",
    # "8eeb84e",
    # "b3f5395",

    "6aec863",  # functional alpha/beta + iterative deepening + material eval with position tables
    "53a09e5",  # move ordering via pv + mvv-lva
    "bbef3be",  # quiescence search
    "35f6fb6",  # transposition table
    "ff14c29",  # faster eval
]


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


def build_fastchess_command(commit_names, add_master=False):
    """
    Build the fastchess command with multiple engines based on commit names.
    """
    # Base command
    cmd = ["./bin/fastchess/fastchess"]

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

    # Add engine parameters for each commit
    for i, commit_name in enumerate(commit_names):
        binary_path = find_binary(commit_name)
        if binary_path is None:
            print(f"Error: Could not find binary for commit {commit_name}")
            sys.exit(1)

        engine_name = commit_name

        # Add -{i} so we can know which ones are the most recent
        cmd.extend([
            "-engine",
            f"cmd={binary_path}",
            f"name={engine_name}-{i}"
        ])

    # Add common parameters
    cmd.extend([
        "-each", "tc=5",
        "-rounds", "300",
        "-concurrency", "32",
        "-config", "outname=scripts/tournament_results.json"
    ])

    return cmd


def run_fastchess(commit_names, add_master=False):
    """
    Run fastchess with the specified commit engines.
    """
    command = build_fastchess_command(commit_names, add_master)

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
    engine_list = sorted(list(engines), key=lambda x: int(x.split('-')[-1]))

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


def main():
    """
    Main function to run the fastchess tournament.
    """
    parser = argparse.ArgumentParser(description="Run fastchess tournament with specified engines")
    parser.add_argument("--add-master", action="store_true",
                       help="Include the current prokopakop binary (master) in the tournament")
    parser.add_argument("--heatmap-only", action="store_true",
                       help="Only generate the heatmap from existing results without running tournament")
    parser.add_argument("--results-file", default="scripts/tournament_results.json",
                       help="Path to the tournament results JSON file")

    args = parser.parse_args()

    if args.heatmap_only:
        # Just generate the heatmap from existing results
        print("Generating heatmap from existing results...")
        generate_heatmap(args.results_file, args.add_master)
    else:
        # Run the tournament
        total_engines = len(COMMIT_NAMES)
        if args.add_master:
            total_engines += 1

        print(f"Setting up fastchess tournament with {total_engines} engines:")

        if args.add_master:
            print("  master (target/release/prokopakop)")

        for i, commit in enumerate(COMMIT_NAMES):
            print(f"  {i}. {commit} (target/release/prokopakop-{commit})")
        print()

        # Run fastchess
        exit_code = run_fastchess(COMMIT_NAMES, args.add_master)

        if exit_code == 0:
            # Generate heatmap after successful tournament
            print("\nGenerating performance heatmap...")
            generate_heatmap(args.results_file, args.add_master)

        sys.exit(exit_code)


if __name__ == "__main__":
    main()
