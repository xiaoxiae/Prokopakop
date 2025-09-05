#!/usr/bin/env python3

import argparse
import subprocess
import sys

from pathlib import Path

# List of commit names - modify this list as needed
# Lower values should correspond to newer commits
COMMIT_NAMES = [
    "9fec3a0",
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
        "-rounds", "1000",
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

def main():
    """
    Main function to run the fastchess tournament.
    """
    parser = argparse.ArgumentParser(description="Run fastchess tournament with specified engines")
    parser.add_argument("--add-master", action="store_true",
                       help="Include the current prokopakop binary (master) in the tournament")

    args = parser.parse_args()

    total_engines = len(COMMIT_NAMES)
    if args.add_master:
        total_engines += 1

    print(f"Setting up fastchess tournament with {total_engines} engines:")

    if args.add_master:
        print("  master (target/release/prokopakop)")

    for i, commit in enumerate(COMMIT_NAMES, 1):
        print(f"  {i}. {commit} (target/release/prokopakop-{commit})")
    print()

    # Run fastchess
    exit_code = run_fastchess(COMMIT_NAMES, args.add_master)
    sys.exit(exit_code)

if __name__ == "__main__":
    main()
