#!/usr/bin/env python3
"""
Chess Engine Benchmark Script - Binary Version

This script benchmarks pre-compiled Rust chess engine binaries by:
1. Looking for binaries in target/release/prokopakop-<name>
2. Running a perft test via UCI using hyperfine
3. Collecting timing data for each version
"""

import subprocess
import time
import re
import json
from datetime import datetime
from pathlib import Path
import sys
import shutil
import matplotlib.pyplot as plt

COMMIT_NAMES = [
    "2bd6f6d",
    "08b45ae",
    "2c8094b",
    "33cbd8e",
    "dc9fc03",
    "a6156ee",
    "f0253ee",
    "d7c5b4d",
    "dda91d3",
    "896a48c",
    "3b5f598",
    "1d8e55a",
    "a303d10",
    "b6128a7",
    "0d9b6b5",
    "c45b84b",
    "a672056",
    "5bc16a6",
    "9902047",
    "25c5d94",
    "cd5c0ad",
    "90ad358",
    "77e73ff",
    "6e73f66",
    "ec94baa",
    "4696ac8",
    "0141f7c",
    "4019af2",
]

class ChessEngineBenchmark:
    def __init__(self, repo_path="."):
        self.repo_path = Path(repo_path).resolve()
        self.results = []

        self.versions_to_test = COMMIT_NAMES

    def run_command(self, cmd, check=True, capture_output=True, timeout=300, cwd=None):
        """Run a shell command and return the result."""
        if cwd is None:
            cwd = self.repo_path
        try:
            result = subprocess.run(
                cmd,
                shell=True,
                check=check,
                capture_output=capture_output,
                text=True,
                cwd=cwd,
                timeout=timeout
            )
            return result
        except subprocess.TimeoutExpired:
            print(f"Command timed out: {cmd}")
            return None
        except subprocess.CalledProcessError as e:
            if check:
                raise
            return e

    def check_hyperfine(self):
        """Check if hyperfine is available."""
        if shutil.which("hyperfine") is None:
            print("Error: hyperfine is not installed or not in PATH")
            print("Please install hyperfine: https://github.com/sharkdp/hyperfine")
            sys.exit(1)
        print("Found hyperfine")

    def find_binary(self, version_name):
        """Find the binary for a given version name."""
        # Extract the suffix after 'prokop-' if version_name starts with 'prokop-'
        binary_prefix = f"prokopakop-{version_name}"
        release_dir = self.repo_path / "target" / "release"

        for binary_path in release_dir.iterdir():
            if binary_path.is_file() and binary_path.name.startswith(binary_prefix):
                return binary_path

        return None

    def validate_perft_output(self, output):
        """Validate that perft output contains the expected node count."""
        expected_nodes = 4865609
        nodes_match = re.search(r'Nodes:\s*(\d+)', output)

        if not nodes_match:
            return False, "No node count found in output"

        nodes = int(nodes_match.group(1))
        if nodes != expected_nodes:
            return False, f"Node count mismatch: expected {expected_nodes}, got {nodes}"

        return True, None

    def measure_single_run_time(self, executable):
        """Measure the time for a single run to calculate target runs for 30 seconds."""
        uci_commands = "uci\\ngo perft 5\\nquit\\n"
        test_cmd = f'printf "{uci_commands}" | {executable}'

        print("Measuring single run time for dynamic run calculation...")
        start_time = time.time()

        result = self.run_command(test_cmd, check=False, timeout=60)

        if not result or result.returncode != 0:
            print(f"Single run measurement failed")
            return None

        single_run_time = time.time() - start_time
        print(f"Single run took {single_run_time:.2f} seconds")

        return single_run_time

    def calculate_target_runs(self, single_run_time, target_duration=30.0, min_runs=5):
        """Calculate the number of runs needed to reach target duration."""
        min_runs = getattr(self, 'min_runs', min_runs)

        if single_run_time <= 0:
            return min_runs

        # Calculate runs needed, accounting for warmup overhead
        target_runs = int(target_duration / single_run_time)

        # Apply bounds
        target_runs = max(min_runs, target_runs)

        estimated_duration = target_runs * single_run_time
        print(f"Calculated {target_runs} runs for ~{estimated_duration:.1f}s total duration")

        return target_runs

    def benchmark_engine(self, executable_path, version_name, target_duration=30.0, warmup_runs=3):
        """Run the perft benchmark using hyperfine with dynamic run count."""
        uci_commands = "uci\\ngo perft 5\\nquit\\n"

        print(f"Benchmarking {version_name} with {executable_path}...")

        # First, validate that the engine produces correct output
        print("Validating engine output...")
        test_cmd = f'printf "{uci_commands}" | {executable_path}'
        test_result = self.run_command(test_cmd, check=False, timeout=30)

        if not test_result or test_result.returncode != 0:
            print(f"Engine test failed for {version_name}")
            return {
                "version": version_name,
                "executable": str(executable_path),
                "error": "test_failed"
            }

        # Validate output
        is_valid, error_msg = self.validate_perft_output(test_result.stdout)
        if not is_valid:
            print(f"Validation failed: {error_msg}")
            return {
                "version": version_name,
                "executable": str(executable_path),
                "error": "validation_failed",
                "error_details": error_msg
            }

        print("Engine validation successful")

        # Measure single run time to calculate target runs
        single_run_time = self.measure_single_run_time(executable_path)
        if single_run_time is None:
            print("Could not measure single run time, using default runs")
            runs = 10
        else:
            runs = self.calculate_target_runs(single_run_time, target_duration)

        # Calculate timeout based on expected duration plus buffer
        expected_duration = runs * (single_run_time or 1.0) + 60  # 60s buffer
        timeout = max(300, int(expected_duration * 1.5))  # At least 5 minutes, or 1.5x expected

        # Run hyperfine benchmark
        hyperfine_cmd = f'hyperfine --runs {runs} --warmup {warmup_runs} --export-json /tmp/hyperfine_results_{version_name}.json \'{test_cmd}\''

        print(f"Running hyperfine with {runs} runs and {warmup_runs} warmup (timeout: {timeout}s)...")
        hyperfine_result = self.run_command(hyperfine_cmd, check=False, timeout=timeout)

        if not hyperfine_result or hyperfine_result.returncode != 0:
            print(f"Hyperfine failed for {version_name}")
            return {
                "version": version_name,
                "executable": str(executable_path),
                "error": "hyperfine_failed"
            }

        # Parse hyperfine JSON results
        try:
            with open(f'/tmp/hyperfine_results_{version_name}.json', 'r') as f:
                hyperfine_data = json.load(f)

            if not hyperfine_data.get('results'):
                print("No results in hyperfine output")
                return {
                    "version": version_name,
                    "executable": str(executable_path),
                    "error": "no_results"
                }

            result = hyperfine_data['results'][0]

            # Extract timing statistics
            mean_time = result['mean']
            std_time = result['stddev']
            min_time = result['min']
            max_time = result['max']
            times = result.get('times', [])

            # Convert to milliseconds and calculate NPS
            mean_time_ms = mean_time * 1000
            std_time_ms = std_time * 1000
            min_time_ms = min_time * 1000
            max_time_ms = max_time * 1000
            times_ms = [t * 1000 for t in times]

            expected_nodes = 4865609
            mean_nps = expected_nodes / mean_time

            return {
                "version": version_name,
                "executable": str(executable_path),
                "nodes": expected_nodes,
                "mean_time_ms": mean_time_ms,
                "std_time_ms": std_time_ms,
                "min_time_ms": min_time_ms,
                "max_time_ms": max_time_ms,
                "mean_nps": mean_nps,
                "runs": runs,
                "warmup_runs": warmup_runs,
                "times_ms": times_ms,
                "single_run_time": single_run_time,
                "target_duration": target_duration,
                "actual_duration": sum(times),
                "hyperfine_data": hyperfine_data
            }

        except Exception as e:
            print(f"Error parsing hyperfine results: {e}")
            return {
                "version": version_name,
                "executable": str(executable_path),
                "error": "parse_failed",
                "error_details": str(e)
            }
        finally:
            # Clean up temporary file
            try:
                Path(f'/tmp/hyperfine_results_{version_name}.json').unlink(missing_ok=True)
            except:
                pass

    def run_benchmark(self, target_duration=30.0, warmup_runs=3):
        """Run the complete benchmark across all specified versions."""
        print("Starting chess engine benchmark...")
        print(f"Working directory: {self.repo_path}")
        print(f"Target duration per version: {target_duration}s")
        print(f"Versions to test: {', '.join(self.versions_to_test)}")

        # Check if hyperfine is available
        self.check_hyperfine()

        # Check which binaries exist
        available_versions = []
        missing_versions = []

        for version in self.versions_to_test:
            binary_path = self.find_binary(version)
            if binary_path:
                available_versions.append((version, binary_path))
                print(f"✓ Found binary for {version}: {binary_path}")
            else:
                missing_versions.append(version)
                print(f"✗ Binary not found for {version}: prokopakop-{version}")

        if missing_versions:
            print(f"\nWarning: Missing binaries for: {', '.join(missing_versions)}")

        if not available_versions:
            print("\nError: No binaries found to test!")
            sys.exit(1)

        print(f"\nWill benchmark {len(available_versions)} version(s)")

        try:
            for idx, (version, binary_path) in enumerate(available_versions, 1):
                print(f"\n--- Version {idx}/{len(available_versions)}: {version} ---")

                # Run benchmark
                benchmark_result = self.benchmark_engine(binary_path, version, target_duration, warmup_runs)

                # Store result
                result_entry = {
                    "version": version,
                    "benchmark": benchmark_result,
                    "timestamp": datetime.now().isoformat()
                }
                self.results.append(result_entry)

                # Print results
                if "error" not in benchmark_result:
                    nodes = benchmark_result.get("nodes", "N/A")
                    mean_time_ms = benchmark_result.get("mean_time_ms", "N/A")
                    std_time_ms = benchmark_result.get("std_time_ms", "N/A")
                    mean_nps = benchmark_result.get("mean_nps", "N/A")
                    runs_count = benchmark_result.get("runs", "N/A")
                    actual_duration = benchmark_result.get("actual_duration", "N/A")

                    print(f"Results ({runs_count} runs, {actual_duration:.1f}s total):")
                    print(f"  Mean: {mean_time_ms:.1f}ms ± {std_time_ms:.1f}ms")
                    print(f"  NPS: {mean_nps:,.0f}")
                else:
                    print(f"Benchmark failed: {benchmark_result.get('error', 'Unknown error')}")
                    if 'error_details' in benchmark_result:
                        print(f"  Details: {benchmark_result['error_details']}")

        except KeyboardInterrupt:
            print("\nBenchmark interrupted by user")
        except Exception as e:
            print(f"Error during benchmark: {e}")

    def save_results(self, filename="benchmark_results.json"):
        """Save results to a JSON file."""
        if not self.results:
            print("No results to save")
            return

        filepath = Path(filename)
        if not filepath.is_absolute():
            filepath = Path(__file__).parent / filename

        with open(filepath, 'w') as f:
            json.dump(self.results, f, indent=2)

        print(f"Results saved to {filepath}")

    def create_plots(self):
        if not self.results:
            print("No results to plot")
            return

        successful_results = [r for r in self.results if "error" not in r["benchmark"]]

        if not successful_results:
            print("No successful results to plot")
            return

        print(f"Creating plots for {len(successful_results)} successful commits...")

        # Extract data for plotting
        commits = []
        nps_values = []

        for result in reversed(successful_results):  # Reverse to show chronological order
            commit_info = result["commit"]
            benchmark = result["benchmark"]

            commits.append(commit_info["hash"][:8])
            nps_values.append(benchmark["mean_nps"])

        x_positions = range(len(commits))

        # NPS plot only
        fig, ax = plt.subplots(1, 1, figsize=(12, 6))
        ax.plot(x_positions, nps_values, marker='s', linewidth=2, markersize=6,
               color='#28B463', markerfacecolor='#58D68D', markeredgecolor='#28B463')
        ax.set_title('Chess Engine Nodes Per Second per Commit', fontsize=14, fontweight='bold')
        ax.set_xlabel('Commits (Chronological Order)', fontsize=12)
        ax.set_ylabel('NPS (Nodes/Second)', fontsize=12)
        ax.grid(True, alpha=0.3)
        ax.set_xticks(x_positions)
        ax.set_xticklabels(commits, rotation=45, ha='right')
        ax.yaxis.set_major_formatter(plt.FuncFormatter(lambda x, p: f'{x:,.0f}'))
        ax.legend()
        plt.tight_layout()
        plt.savefig("benchmark.png", dpi=300, bbox_inches='tight')
        plt.close(fig)

    def print_summary(self):
        """Print a summary of benchmark results."""
        if not self.results:
            print("No results to summarize")
            return

        print(f"\n--- Benchmark Summary ({len(self.results)} versions) ---")
        print(f"{'Version':<20} {'Status':<15} {'Runs':<6} {'Duration':<10} {'Mean Time':<15} {'±Std':<12} {'NPS':<15}")
        print("-" * 100)

        for result in self.results:
            version = result["version"]
            benchmark = result["benchmark"]

            if "error" in benchmark:
                status = benchmark["error"].replace("_", " ").title()
                runs = str(benchmark.get('runs', '0'))
                print(f"{version:<20} {status:<15} {runs:<6} {'N/A':<10} {'N/A':<15} {'N/A':<12} {'N/A':<15}")
            else:
                runs = str(benchmark.get("runs", "1"))
                duration = f"{benchmark.get('actual_duration', 0):.1f}s" if 'actual_duration' in benchmark else "N/A"
                mean_time_ms = benchmark.get("mean_time_ms", "N/A")
                std_time_ms = benchmark.get("std_time_ms", "N/A")
                mean_nps = benchmark.get("mean_nps", "N/A")

                mean_str = f"{mean_time_ms:.1f}ms" if isinstance(mean_time_ms, (int, float)) else str(mean_time_ms)
                std_str = f"±{std_time_ms:.1f}ms" if isinstance(std_time_ms, (int, float)) else str(std_time_ms)
                nps_str = f"{mean_nps:,.0f}" if isinstance(mean_nps, (int, float)) else str(mean_nps)
                print(f"{version:<20} {'Success':<15} {runs:<6} {duration:<10} {mean_str:<15} {std_str:<12} {nps_str:<15}")

def main():
    """Main function to run the benchmark."""
    import argparse

    parser = argparse.ArgumentParser(description="Benchmark chess engine binaries using hyperfine")
    parser.add_argument("--repo", "-r", default=None, help="Repository path (default: parent directory if script is in scripts/, otherwise current directory)")
    parser.add_argument("--duration", "-d", type=float, default=30.0, help="Target duration for benchmarks in seconds (default: 30.0)")
    parser.add_argument("--warmup", "-w", type=int, default=3, help="Number of warmup runs (default: 3)")
    parser.add_argument("--min-runs", type=int, default=10, help="Minimum number of runs per version (default: 10)")
    parser.add_argument("--plots-dir", default="plots", help="Directory for saving plots (default: plots)")
    parser.add_argument("--output", "-o", default="benchmark_results.json", help="Output file for results")
    parser.add_argument("--versions", "-v", nargs="+", help="Override the list of versions to test")

    args = parser.parse_args()

    # Determine repo path
    if args.repo is None:
        # Default to parent directory if script is in scripts/
        script_dir = Path(__file__).parent
        if script_dir.name == "scripts":
            repo_path = script_dir.parent
        else:
            repo_path = "."
    else:
        repo_path = args.repo

    # Check if Cargo.toml exists (to verify it's a Rust project)
    if not Path(repo_path, "Cargo.toml").exists():
        print(f"Warning: {repo_path} does not appear to be a Rust project (no Cargo.toml found)")

    benchmark = ChessEngineBenchmark(repo_path)

    # Override versions list if provided via command line
    if args.versions:
        benchmark.versions_to_test = args.versions
        print(f"Using custom version list: {', '.join(args.versions)}")

    # Store command line arguments for the benchmark
    benchmark.target_duration = args.duration
    benchmark.min_runs = args.min_runs

    try:
        benchmark.run_benchmark(args.duration, args.warmup)

        benchmark.print_summary()
        benchmark.save_results(args.output)
        benchmark.create_plots()
    except Exception as e:
        print(f"Benchmark failed: {e}")
        # Still try to save any results we got
        if benchmark.results:
            benchmark.save_results(args.output)
        sys.exit(1)

if __name__ == "__main__":
    main()
