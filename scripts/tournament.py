#!/usr/bin/env python3

import argparse
import shlex
import subprocess
import sys
from pathlib import Path

# Tournament configuration constants
TIME_CONTROL = "30+0.1"
ROUNDS = 100
CONCURRENCY = 32
OPENING_PLIES = 6

BINARIES: list[str | dict[str, str | dict[str, str]]] = [
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
    # "da6dbb0",  # aspiration windows
    # "7cfbb74",  # PV search
    # "5ddad41",  # bucket TT
    # "6e3bad1",  # history heuristic
    # "c14002b",  # pseudo-legal move generation
    # "e6662f0",  # LMR bug
    # "075f411",  # actually, no pseudo-legal move generation
    # "7490eba",  # futility pruning
    # "8024a6e",  # no partial result usage + better iterative deepening time management
    # "c144fb9",  # no TT cleaning between moves
    # "5cc228e",  # king safety
    # "910fb21",  # DP + NMP tuning
    # "c056f9b",  # SEE
    "9119428",  # razoring
    "3f0d5f5",  # good NNUE
    "fb7ccfe",
    # {"label": "current", "options": {"NNUE": "data/nnue.bin"}},
    # {
    #     "label": "experiment-9-0",
    #     "options": {
    #         "NNUE": "train/experiment-9/wdl_0.25_lr_0.00100_gamma_0.10_0/experiment-100/quantised.bin"
    #     },
    # },
    # {
    #     "label": "experiment-9-1",
    #     "options": {"NNUE": "train/experiment-9/wdl_0.25_lr_0.00100_gamma_0.10_1/experiment-100/quantised.bin"}
    # },
    # {
    #     "label": "experiment-10-0",
    #     "options": {"NNUE": "train/experiment-10/wdl_0.25_lr_0.00100_gamma_0.50_0/experiment-240/quantised.bin"}
    # },
    # {
    #     "label": "experiment-10-1",
    #     "options": {"NNUE": "train/experiment-10/wdl_0.25_lr_0.00100_gamma_0.50_1/experiment-240/quantised.bin"}
    # },
    # {
    #     "label": "experiment-11-0",
    #     "options": {"NNUE": "train/experiment-11/wdl_0.25_lr_0.00100_gamma_0.50_0/experiment-240/quantised.bin"}
    # },
    # {
    #     "label": "experiment-11-1",
    #     "options": {"NNUE": "train/experiment-11/wdl_0.25_lr_0.00100_gamma_0.50_1/experiment-240/quantised.bin"}
    # },
    ## {
    ##     "label": "experiment-12-0",
    ##     "options": {
    ##         "NNUE": "train/experiment-12/wdl_0.25_lr_0.00100_gamma_0.50_0/experiment-240/quantised.bin"
    ##     },
    ## },
    # {
    #     "label": "experiment-12-1",
    #     "options": {
    #         "NNUE": "train/experiment-12/wdl_0.25_lr_0.00100_gamma_0.50_1/experiment-240/quantised.bin"
    #     },
    # },
    # {
    #     "label": "experiment-12-2",
    #     "options": {
    #         "NNUE": "train/experiment-12/wdl_0.25_lr_0.00100_gamma_0.50_2/experiment-240/quantised.bin"
    #     },
    # },
    ## {
    ##     "label": "experiment-13-0",
    ##     "options": {
    ##         "NNUE": "train/experiment-13/wdl_0.25_lr_0.00100_gamma_0.50_0/experiment-600/quantised.bin"
    ##     },
    ## },
    # {
    #     "label": "experiment-13-1",
    #     "options": {
    #         "NNUE": "train/experiment-13/wdl_0.25_lr_0.00100_gamma_0.50_1/experiment-600/quantised.bin"
    #     },
    # },
    # {
    #     "label": "experiment-13-2",
    #     "options": {"NNUE": "train/experiment-13/wdl_0.25_lr_0.00100_gamma_0.50_2/experiment-600/quantised.bin"}
    # },
    # {
    #     "label": "experiment-13-3",
    #     "options": {"NNUE": "train/experiment-13/wdl_0.25_lr_0.00100_gamma_0.50_3/experiment-600/quantised.bin"}
    # },
    ## {
    ##     "label": "experiment-14-0",
    ##     "options": {
    ##         "NNUE": "train/experiment-14/wdl_0.25_lr_0.00100_gamma_0.50_0/experiment-240/quantised.bin"
    ##     },
    ## },
    ## {
    ##     "label": "experiment-14-1",
    ##     "options": {
    ##         "NNUE": "train/experiment-14/wdl_0.25_lr_0.00100_gamma_0.50_1/experiment-240/quantised.bin"
    ##     },
    ## },
    # {
    #     "label": "experiment-14-2",
    #     "options": {"NNUE": "train/experiment-14/wdl_0.25_lr_0.00100_gamma_0.50_2/experiment-240/quantised.bin"}
    # },
    # {
    #     "label": "experiment-14-3",
    #     "options": {"NNUE": "train/experiment-14/wdl_0.25_lr_0.00100_gamma_0.50_3/experiment-240/quantised.bin"}
    # },
    ## {
    ##     "label": "experiment-15-0",
    ##     "options": {
    ##         "NNUE": "train/experiment-15/wdl_0.25_lr_0.00100_gamma_0.50_0/experiment-240/quantised.bin"
    ##     },
    ## },
    ## {
    ##     "label": "experiment-15-1",
    ##     "options": {
    ##         "NNUE": "train/experiment-15/wdl_0.25_lr_0.00100_gamma_0.50_1/experiment-240/quantised.bin"
    ##     },
    ## },
    # {
    #     "label": "experiment-15-2",
    #     "options": {
    #         "NNUE": "train/experiment-15/wdl_0.25_lr_0.00100_gamma_0.50_2/experiment-240/quantised.bin"
    #     },
    # },
    # {
    #     "label": "experiment-15-3",
    #     "options": {
    #         "NNUE": "train/experiment-15/wdl_0.25_lr_0.00100_gamma_0.50_3/experiment-240/quantised.bin"
    #     },
    # },
    # {
    #     "label": "experiment-15-4",
    #     "options": {
    #         "NNUE": "train/experiment-15/wdl_0.25_lr_0.00100_gamma_0.50_4/experiment-240/quantised.bin"
    #     },
    # },
    # {
    #     "label": "experiment-16-0",
    #     "options": {
    #         "NNUE": "train/experiment-16/wdl_0.25_lr_0.00100_gamma_0.50_0/experiment-240/quantised.bin"
    #     },
    # },
    # {
    #     "label": "experiment-16-1",
    #     "options": {
    #         "NNUE": "train/experiment-16/wdl_0.25_lr_0.00100_gamma_0.50_1/experiment-240/quantised.bin"
    #     },
    # },
    # {
    #     "label": "experiment-16-2",
    #     "options": {
    #         "NNUE": "train/experiment-16/wdl_0.25_lr_0.00100_gamma_0.50_2/experiment-240/quantised.bin"
    #     },
    # },
    # {
    #     "label": "experiment-16-3",
    #     "options": {
    #         "NNUE": "train/experiment-16/wdl_0.25_lr_0.00100_gamma_0.50_3/experiment-240/quantised.bin"
    #     },
    # },
    # {
    #     "label": "experiment-17-0",
    #     "options": {
    #         "NNUE": "train/experiment-17/wdl_0.25_lr_0.00100_gamma_0.50_0/experiment-40/quantised.bin"
    #     },
    # },
    {
        "label": "experiment-18-0",
        "options": {
            "NNUE": "train/experiment-18/wdl_0.25_lr_0.00100_gamma_0.50_0/experiment-240/quantised.bin"
        },
    },
    {
        "label": "experiment-18-1",
        "options": {
            "NNUE": "train/experiment-18/wdl_0.25_lr_0.00100_gamma_0.50_1/experiment-240/quantised.bin"
        },
    },
    {
        "label": "experiment-18-2",
        "options": {
            "NNUE": "train/experiment-18/wdl_0.25_lr_0.00100_gamma_0.50_2/experiment-240/quantised.bin"
        },
    },
    {
        "label": "experiment-18-3",
        "options": {
            "NNUE": "train/experiment-18/wdl_0.25_lr_0.00100_gamma_0.50_3/experiment-240/quantised.bin"
        },
    },
    {
        "label": "experiment-18-4",
        "options": {
            "NNUE": "train/experiment-18/wdl_0.25_lr_0.00100_gamma_0.50_4/experiment-240/quantised.bin"
        },
    },
]


def find_binary(binary_name: str) -> Path | None:
    """Find the binary for a given binary name."""
    # Empty name means use the default prokopakop binary
    if binary_name == "":
        binary_name = "prokopakop"

    release_dir: Path = Path("target") / "release"

    # Check if binary exists directly (for prokopakop or other exact names)
    binary_path: Path = release_dir / binary_name
    if binary_path.is_file():
        return binary_path

    # Check with prokopakop- prefix (for commit hashes)
    binary_prefix: str = f"prokopakop-{binary_name}"
    for candidate in release_dir.iterdir():
        if candidate.is_file() and candidate.name.startswith(binary_prefix):
            return candidate

    return None


def get_binary_info(
    binary_item: str | dict[str, str | dict[str, str]],
) -> tuple[str, str | None, dict[str, str]]:
    """Extract binary name, label, and options from binary item (supports both string and dict formats)."""
    if isinstance(binary_item, str):
        return binary_item, None, {}
    elif isinstance(binary_item, dict):
        # If no name is provided, use empty string (which means use default prokopakop)
        binary_name: str = binary_item.get("name", "")
        label: str | None = binary_item.get("label", None)
        options: dict[str, str] = binary_item.get("options", {})
        return binary_name, label, options
    else:
        raise ValueError(f"Invalid binary format: {binary_item}")


def add_uci_options(cmd: list[str], options: dict[str, str]) -> None:
    """Add UCI options to the command list."""
    for option_name, option_value in options.items():
        cmd.extend([f"option.{option_name}={option_value}"])


def build_fastchess_command(
    binaries: list[str | dict[str, str | dict[str, str]]],
) -> list[str]:
    """
    Build the fastchess command with multiple engines based on binaries.

    Args:
        binaries: List of binaries to include
    """
    # Base command
    cmd: list[str] = ["./bin/fastchess/fastchess"]

    # Add engine parameters for each binary
    for i, binary_item in enumerate(binaries):
        binary_name: str
        label: str | None
        binary_options: dict[str, str]
        binary_name, label, binary_options = get_binary_info(binary_item)

        binary_path: Path | None = find_binary(binary_name)
        if binary_path is None:
            print(f"Error: Could not find binary for {binary_name}")
            sys.exit(1)

        # Use label if provided, otherwise use binary_name (or "prokopakop" if empty)
        if label:
            engine_name: str = label
        else:
            engine_name = binary_name if binary_name else "prokopakop"
        engine_cmd: str = str(binary_path)

        cmd.extend(["-engine", f"cmd={engine_cmd}", f"name={engine_name}-{i}"])
        # Add UCI options for this binary
        add_uci_options(cmd, binary_options)

    # Add common parameters
    cmd.extend(
        [
            "-each",
            f"tc={TIME_CONTROL}",
            "restart=on",
            "-rounds",
            str(ROUNDS),
            "-concurrency",
            str(CONCURRENCY),
            "-config",
            "outname=scripts/tournament_results.json",
            "-openings",
            "file=data/book.pgn",
            "format=pgn",
            f"plies={OPENING_PLIES}",
            "order=random",
        ]
    )

    # Wrap command with grep filter to remove noisy lines
    cmd_str: str = " ".join(shlex.quote(arg) for arg in cmd)
    filtered_cmd: str = f"{cmd_str} | grep -Ev '^(Moves|Info|Warning|Position);'"

    return ["bash", "-c", filtered_cmd]


def run_fastchess(binaries: list[str | dict[str, str | dict[str, str]]]) -> int:
    """
    Run fastchess with the specified binaries.

    Args:
        binaries: List of binaries to include
    """
    command: list[str] = build_fastchess_command(binaries)

    print("Running fastchess with the following command:")
    print(" ".join(command))
    print()

    try:
        # Run the command
        result: subprocess.CompletedProcess[str] = subprocess.run(
            command, check=True, text=True
        )
        print("Fastchess completed successfully!")
        return result.returncode
    except subprocess.CalledProcessError as e:
        print(f"Error running fastchess: {e}")
        return e.returncode
    except FileNotFoundError:
        print("Error: fastchess binary not found at ./bin/fastchess/fastchess")
        print("Please ensure the binary exists and is executable.")
        return 1


def print_engine_info(binaries: list[str | dict[str, str | dict[str, str]]]) -> None:
    """Print information about engines and their options."""
    total_engines: int = len(binaries)

    print(f"Engines configured ({total_engines} total):")

    for i, binary_item in enumerate(binaries):
        binary_name: str
        label: str | None
        binary_options: dict[str, str]
        binary_name, label, binary_options = get_binary_info(binary_item)
        options_str: str = ""
        if binary_options:
            options_list: list[str] = [f"{k}={v}" for k, v in binary_options.items()]
            options_str = f" (options: {', '.join(options_list)})"

        # Use label if provided, otherwise use binary_name (or "prokopakop" if empty)
        if label:
            display_name: str = label
        else:
            display_name = binary_name if binary_name else "prokopakop"

        if binary_name:
            path_str: str = f"target/release/{binary_name}"
        else:
            path_str = "target/release/prokopakop"

        print(f"  {i}. {display_name} ({path_str}){options_str}")


def main() -> None:
    """
    Main function to run the fastchess tournament.
    """
    parser: argparse.ArgumentParser = argparse.ArgumentParser(
        description="Run fastchess tournament with specified engines"
    )

    args: argparse.Namespace = parser.parse_args()

    print("Setting up fastchess tournament:")
    print_engine_info(BINARIES)
    print()

    # Run fastchess
    exit_code: int = run_fastchess(BINARIES)
    sys.exit(exit_code)


if __name__ == "__main__":
    main()
