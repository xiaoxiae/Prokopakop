#!/bin/bash

# Build script for compiling different NNUE versions of prokopakop
# For each model in train/checkpoints, builds the engine with that NNUE and
# copies the binary to target/release/prokopakop-nnue-<model_name>-<checkpoint>
#
# Usage: ./build-engines.sh [INTERVAL]
#   INTERVAL: Optional. If specified, builds every INTERVAL-th checkpoint starting
#             from the last one (e.g., 30 builds checkpoints 90, 60, 30 for a model
#             with checkpoints 0-90). If not specified, builds only the latest.

set -e

# Parse command line arguments
INTERVAL=${1:-0}  # Default to 0 (only latest checkpoint)

# Get the directory where this script is located
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
DATA_DIR="$PROJECT_ROOT/data"
TARGET_DIR="$PROJECT_ROOT/target/release"
CHECKPOINTS_DIR="$PROJECT_ROOT/train/experiment-0"

# Ensure data directory exists
mkdir -p "$DATA_DIR"

# Array to store built engine names for final output
declare -a BUILT_ENGINES

# Find all model directories (those containing checkpoint subdirectories)
find "$CHECKPOINTS_DIR" -mindepth 1 -maxdepth 1 -type d | sort | while read model_dir; do
    model_name=$(basename "$model_dir")

    # Find all checkpoints (directories with quantised.bin), sorted by version
    mapfile -t all_checkpoints < <(find "$model_dir" -mindepth 1 -maxdepth 1 -type d | sort -V)

    if [ ${#all_checkpoints[@]} -eq 0 ]; then
        echo "Skipping $model_name: no checkpoints found"
        continue
    fi

    # Select checkpoints based on INTERVAL
    declare -a selected_checkpoints
    if [ "$INTERVAL" -eq 0 ]; then
        # Just use the latest checkpoint
        selected_checkpoints=("${all_checkpoints[-1]}")
    else
        # Select every INTERVAL-th checkpoint starting from the last
        for ((i=${#all_checkpoints[@]}-1; i>=0; i-=INTERVAL)); do
            selected_checkpoints+=("${all_checkpoints[i]}")
        done
    fi

    # Build engine for each selected checkpoint
    for checkpoint_dir in "${selected_checkpoints[@]}"; do
        checkpoint_num=$(basename "$checkpoint_dir")
        quantised_file="$checkpoint_dir/quantised.bin"

        if [ ! -f "$quantised_file" ]; then
            echo "Skipping checkpoint $checkpoint_num: no quantised.bin found"
            continue
        fi

        echo "Building engine for model: $model_name, checkpoint: $checkpoint_num"
        echo "Source: $quantised_file"

        # Copy the quantised.bin to data/nnue.bin
        cp "$quantised_file" "$DATA_DIR/nnue.bin"
        echo "Copied to $DATA_DIR/nnue.bin"

        # Build the engine
        echo "Building with 'cargo build --release'..."
        cd "$PROJECT_ROOT"
        cargo build --release

        # Copy the binary to target/release/prokopakop-nnue-<model_name>-<checkpoint_num>
        binary_name="prokopakop-nnue-$model_name-$checkpoint_num"
        cp "$TARGET_DIR/prokopakop" "$TARGET_DIR/$binary_name"
        echo "Copied binary to $TARGET_DIR/$binary_name"

        BUILT_ENGINES+=("$model_name-$checkpoint_num")
        echo ""
    done
done

echo "Build complete! All engine versions have been compiled."
echo ""
echo "Add the following to COMMIT_NAMES in scripts/tournament.py:"
echo ""

# Print all built engines
if [ "$INTERVAL" -eq 0 ]; then
    # For backward compatibility, print just model names when only latest is built
    find "$CHECKPOINTS_DIR" -mindepth 1 -maxdepth 1 -type d | sort | while read model_dir; do
        model_name=$(basename "$model_dir")
        # Find the latest checkpoint number
        latest_checkpoint=$(find "$model_dir" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -1)
        if [ -n "$latest_checkpoint" ]; then
            checkpoint_num=$(basename "$latest_checkpoint")
            echo "    \"$model_name-$checkpoint_num\","
        fi
    done
else
    # When using intervals, print all model-checkpoint combinations
    find "$CHECKPOINTS_DIR" -mindepth 1 -maxdepth 1 -type d | sort | while read model_dir; do
        model_name=$(basename "$model_dir")
        mapfile -t all_checkpoints < <(find "$model_dir" -mindepth 1 -maxdepth 1 -type d | sort -V)

        if [ ${#all_checkpoints[@]} -eq 0 ]; then
            continue
        fi

        # Select checkpoints based on INTERVAL (same logic as above)
        for ((i=${#all_checkpoints[@]}-1; i>=0; i-=INTERVAL)); do
            checkpoint_num=$(basename "${all_checkpoints[i]}")
            if [ -f "${all_checkpoints[i]}/quantised.bin" ]; then
                echo "    \"nnue-$model_name-$checkpoint_num\","
            fi
        done
    done
fi
