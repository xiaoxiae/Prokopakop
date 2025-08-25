#!/usr/bin/env python3
"""
WARNING -- USES GIT OPERATIONS
DON'T USE THE REPOSITORY WHILE THIS IS RUNNING

Chess Engine Benchmark Script

This script benchmarks a Rust chess engine across git commits by:
1. Compiling the engine in release mode
2. Running a perft 4 test via UCI using hyperfine
3. Going back one commit and repeating
4. Collecting timing data for each commit
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
import matplotlib.dates as mdates
from datetime import datetime as dt

class ChessEngineBenchmark:
    def __init__(self, repo_path="."):
        self.repo_path = Path(repo_path).resolve()
        self.results = []
        self.original_branch = None
        
    def run_command(self, cmd, check=True, capture_output=True, timeout=300):
        """Run a shell command and return the result."""
        try:
            result = subprocess.run(
                cmd, 
                shell=True, 
                check=check, 
                capture_output=capture_output,
                text=True,
                cwd=self.repo_path,
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
    
    def get_current_commit(self):
        """Get the current git commit hash and message."""
        try:
            hash_result = self.run_command("git rev-parse HEAD")
            commit_hash = hash_result.stdout.strip()
            
            msg_result = self.run_command("git log -1 --pretty=format:'%s'")
            commit_msg = msg_result.stdout.strip().strip("'")
            
            date_result = self.run_command("git log -1 --pretty=format:'%ci'")
            commit_date = date_result.stdout.strip().strip("'")
            
            return {
                "hash": commit_hash,
                "message": commit_msg,
                "date": commit_date
            }
        except:
            return None
    
    def save_original_state(self):
        """Save the current branch/commit to restore later."""
        try:
            result = self.run_command("git branch --show-current")
            self.original_branch = result.stdout.strip()
            if not self.original_branch:
                # We're in detached HEAD state
                result = self.run_command("git rev-parse HEAD")
                self.original_branch = result.stdout.strip()
        except:
            print("Warning: Could not determine original git state")
    
    def restore_original_state(self):
        """Restore the original branch/commit."""
        if self.original_branch:
            try:
                self.run_command(f"git checkout {self.original_branch}")
                print(f"Restored to original state: {self.original_branch}")
            except:
                print("Warning: Could not restore original git state")
    
    def compile_engine(self):
        """Compile the Rust chess engine in release mode."""
        print("Compiling engine in release mode...")
        result = self.run_command("cargo build --release", timeout=600)
        return result.returncode == 0 if result else False
    
    def find_executables(self):
        """Find available executables by reading Cargo.toml."""
        cargo_toml_path = self.repo_path / "Cargo.toml"
        
        if not cargo_toml_path.exists():
            print("Cargo.toml not found")
            return None
        
        try:
            # Read Cargo.toml and extract the package name
            with open(cargo_toml_path, 'r') as f:
                content = f.read()
            
            # Look for name = "..." in [package] section
            # Simple regex to find the name field
            name_match = re.search(r'name\s*=\s*["\']([^"\']+)["\']', content)
            
            if not name_match:
                print("Could not find package name in Cargo.toml")
                return None
            
            package_name = name_match.group(1)
            print(f"Found package name: {package_name}")
            
            # Check if the executable exists
            executable_path = f"./target/release/{package_name}"
            full_path = self.repo_path / executable_path.lstrip("./")
            
            if full_path.exists():
                print(f"Found executable: {executable_path}")
                return [executable_path]
            else:
                print(f"Executable not found at: {executable_path}")
                return None
                
        except Exception as e:
            print(f"Error reading Cargo.toml: {e}")
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
        # Use instance variables if available, otherwise use defaults
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
    
    def benchmark_engine(self, executables, target_duration=30.0, warmup_runs=3):
        """Run the perft benchmark using hyperfine with dynamic run count."""
        uci_commands = "uci\\ngo perft 5\\nquit\\n"
        
        # Try each executable in order
        for executable in executables:
            print(f"Benchmarking with {executable} using hyperfine...")
            
            # First, validate that the engine produces correct output
            print("Validating engine output...")
            test_cmd = f'printf "{uci_commands}" | {executable}'
            test_result = self.run_command(test_cmd, check=False, timeout=30)
            
            if not test_result or test_result.returncode != 0:
                print(f"Engine test failed with {executable}")
                continue
            
            # Validate output
            is_valid, error_msg = self.validate_perft_output(test_result.stdout)
            if not is_valid:
                print(f"Validation failed: {error_msg}")
                return {
                    "executable": executable,
                    "error": "validation_failed",
                    "error_details": error_msg,
                    "runs": 1
                }
            
            print("Engine validation successful")
            
            # Measure single run time to calculate target runs
            single_run_time = self.measure_single_run_time(executable)
            if single_run_time is None:
                print("Could not measure single run time, using default runs")
                runs = 10
            else:
                runs = self.calculate_target_runs(single_run_time, target_duration)
            
            # Calculate timeout based on expected duration plus buffer
            expected_duration = runs * (single_run_time or 1.0) + 60  # 60s buffer
            timeout = max(300, int(expected_duration * 1.5))  # At least 5 minutes, or 1.5x expected
            
            # Run hyperfine benchmark
            hyperfine_cmd = f'hyperfine --runs {runs} --warmup {warmup_runs} --export-json /tmp/hyperfine_results.json \'{test_cmd}\''
            
            print(f"Running hyperfine with {runs} runs and {warmup_runs} warmup (timeout: {timeout}s)...")
            hyperfine_result = self.run_command(hyperfine_cmd, check=False, timeout=timeout)
            
            if not hyperfine_result or hyperfine_result.returncode != 0:
                print(f"Hyperfine failed with {executable}")
                continue
            
            # Parse hyperfine JSON results
            try:
                with open('/tmp/hyperfine_results.json', 'r') as f:
                    hyperfine_data = json.load(f)
                
                if not hyperfine_data.get('results'):
                    print("No results in hyperfine output")
                    continue
                
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
                    "executable": executable,
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
                continue
        
        return None
    
    def go_back_one_commit(self):
        """Go back one commit and apply hotfix. Returns False if we can't go back further."""
        try:
            result = self.run_command("git checkout HEAD~", check=False)
            if result.returncode != 0:
                return False
            
            # Hotfix: checkout magic.rs from master if it exists
            self.apply_magic_hotfix()
            return True
        except:
            return False
    
    def apply_magic_hotfix(self):
        """Apply hotfix by checking out magic.rs from master."""
        try:
            # Check if magic.rs exists in master
            result = self.run_command("git show master:src/utils/magic.rs", check=False)
            if result.returncode == 0:
                print("Applying magic.rs hotfix from master...")
                # Checkout magic.rs from master
                hotfix_result = self.run_command("git checkout master -- src/utils/magic.rs", check=False)
                if hotfix_result.returncode == 0:
                    print("Successfully applied magic.rs hotfix")
                else:
                    print("Warning: Failed to apply magic.rs hotfix")
            else:
                # magic.rs doesn't exist in master, that's fine
                pass
        except Exception as e:
            print(f"Warning: Error applying magic.rs hotfix: {e}")
    
    def run_benchmark(self, target_duration=30.0, warmup_runs=3):
        """Run the complete benchmark across commits."""
        print("Starting chess engine benchmark...")
        print(f"Working directory: {self.repo_path}")
        print(f"Target duration per commit: {target_duration}s")
        
        # Check if hyperfine is available
        self.check_hyperfine()
        
        # Save original state
        self.save_original_state()
        
        try:
            commit_count = 0
            
            while True:
                commit_count += 1
                print(f"\n--- Commit {commit_count} ---")
                
                # Get current commit info
                commit_info = self.get_current_commit()
                if not commit_info:
                    print("Could not get commit information")
                    break
                
                print(f"Commit: {commit_info['hash'][:8]} - {commit_info['message']}")
                
                # Compile engine
                compilation_success = self.compile_engine()
                if not compilation_success:
                    print("Compilation failed, skipping this commit")
                    # Store failed result
                    result_entry = {
                        "commit": commit_info,
                        "benchmark": {"error": "compilation_failed"},
                        "timestamp": datetime.now().isoformat()
                    }
                    self.results.append(result_entry)
                else:
                    # Find executables
                    executables = self.find_executables()
                    if not executables:
                        print("Could not find any executables, skipping this commit")
                        # Store failed result
                        result_entry = {
                            "commit": commit_info,
                            "benchmark": {"error": "executable_not_found"},
                            "timestamp": datetime.now().isoformat()
                        }
                        self.results.append(result_entry)
                    else:
                        print(f"Found executables: {executables}")
                        
                        # Run benchmark with dynamic runs
                        benchmark_result = self.benchmark_engine(executables, target_duration, warmup_runs)
                        if not benchmark_result:
                            print("All benchmarks failed, skipping this commit")
                            # Store failed result
                            result_entry = {
                                "commit": commit_info,
                                "benchmark": {"error": "benchmark_failed"},
                                "timestamp": datetime.now().isoformat()
                            }
                            self.results.append(result_entry)
                        elif "error" in benchmark_result:
                            print(f"Benchmark validation failed: {benchmark_result['error']}")
                            # Store validation failure
                            result_entry = {
                                "commit": commit_info,
                                "benchmark": benchmark_result,
                                "timestamp": datetime.now().isoformat()
                            }
                            self.results.append(result_entry)
                        else:
                            # Store successful result
                            result_entry = {
                                "commit": commit_info,
                                "benchmark": benchmark_result,
                                "timestamp": datetime.now().isoformat()
                            }
                            self.results.append(result_entry)
                            
                            # Print results
                            nodes = benchmark_result.get("nodes", "N/A")
                            mean_time_ms = benchmark_result.get("mean_time_ms", "N/A")
                            std_time_ms = benchmark_result.get("std_time_ms", "N/A")
                            mean_nps = benchmark_result.get("mean_nps", "N/A")
                            runs_count = benchmark_result.get("runs", "N/A")
                            executable = benchmark_result.get("executable", "unknown")
                            actual_duration = benchmark_result.get("actual_duration", "N/A")
                            
                            print(f"Results ({executable}, {runs_count} runs, {actual_duration:.1f}s total):")
                            print(f"  Mean: {mean_time_ms:.1f}ms ± {std_time_ms:.1f}ms")
                            print(f"  NPS: {mean_nps:,.0f}")

                # Go back one commit
                if not self.go_back_one_commit():
                    print("Cannot go back further, benchmark complete")
                    break
                    
        except KeyboardInterrupt:
            print("\nBenchmark interrupted by user")
        except Exception as e:
            print(f"Error during benchmark: {e}")
        finally:
            # Restore original state
            self.restore_original_state()
            # Clean up temporary file
            try:
                Path('/tmp/hyperfine_results.json').unlink(missing_ok=True)
            except:
                pass
    
    def save_results(self, filename="benchmark_results.json"):
        """Save results to a JSON file."""
        if not self.results:
            print("No results to save")
            return
        
        filepath = Path(__file__).parent / filename
        with open(filepath, 'w') as f:
            json.dump(self.results, f, indent=2)
        
        print(f"Results saved to {filepath}")
    
    def create_plots(self, output_dir="plots"):
        """Create matplotlib plots showing runtime and NPS trends across commits."""
        if not self.results:
            print("No results to plot")
            return
        
        # Filter successful results
        successful_results = [r for r in self.results if "error" not in r["benchmark"]]
        
        if not successful_results:
            print("No successful results to plot")
            return
        
        print(f"Creating plots for {len(successful_results)} successful commits...")
        
        # Create output directory
        plot_dir = Path(__file__).parent / output_dir
        plot_dir.mkdir(exist_ok=True)
        
        # Extract data for plotting
        commits = []
        commit_labels = []
        runtimes = []
        runtime_stds = []
        nps_values = []
        commit_dates = []
        run_counts = []
        
        for result in reversed(successful_results):  # Reverse to show chronological order
            commit_info = result["commit"]
            benchmark = result["benchmark"]
            
            commits.append(commit_info["hash"][:8])
            commit_labels.append(f"{commit_info['hash'][:8]}\n{commit_info['message'][:20]}...")
            runtimes.append(benchmark["mean_time_ms"])
            runtime_stds.append(benchmark["std_time_ms"])
            nps_values.append(benchmark["mean_nps"])
            run_counts.append(benchmark.get("runs", 0))
            
            # Parse commit date
            try:
                commit_date = dt.fromisoformat(commit_info["date"].replace(' +', '+').replace(' -', '-'))
                commit_dates.append(commit_date)
            except:
                commit_dates.append(dt.now())  # Fallback
        
        # Create figure with subplots
        fig, (ax1, ax2, ax3) = plt.subplots(3, 1, figsize=(14, 12))
        fig.suptitle('Chess Engine Performance Across Git Commits (Dynamic Run Counts)', fontsize=16, fontweight='bold')
        
        x_positions = range(len(commits))
        
        # Plot 1: Runtime with error bars
        ax1.errorbar(x_positions, runtimes, yerr=runtime_stds, 
                    marker='o', linewidth=2, markersize=6, capsize=4, 
                    color='#2E86C1', ecolor='#5DADE2', capthick=2)
        ax1.set_title('Runtime per Commit (Lower is Better)', fontsize=14, fontweight='bold')
        ax1.set_ylabel('Runtime (ms)', fontsize=12)
        ax1.grid(True, alpha=0.3)
        ax1.set_xticks(x_positions)
        ax1.set_xticklabels(commits, rotation=45, ha='right')
        
        # Plot 2: NPS (Nodes Per Second)
        ax2.plot(x_positions, nps_values, marker='s', linewidth=2, markersize=6, 
                color='#28B463', markerfacecolor='#58D68D', markeredgecolor='#28B463')
        ax2.set_title('Nodes Per Second per Commit (Higher is Better)', fontsize=14, fontweight='bold')
        ax2.set_ylabel('NPS (Nodes/Second)', fontsize=12)
        ax2.grid(True, alpha=0.3)
        ax2.set_xticks(x_positions)
        ax2.set_xticklabels(commits, rotation=45, ha='right')
        
        # Format NPS y-axis with commas
        ax2.yaxis.set_major_formatter(plt.FuncFormatter(lambda x, p: f'{x:,.0f}'))
        
        # Plot 3: Run counts per commit
        ax3.bar(x_positions, run_counts, color='#E67E22', alpha=0.7, edgecolor='#D35400')
        ax3.set_title('Hyperfine Run Counts per Commit', fontsize=14, fontweight='bold')
        ax3.set_xlabel('Commits (Chronological Order)', fontsize=12)
        ax3.set_ylabel('Number of Runs', fontsize=12)
        ax3.grid(True, alpha=0.3, axis='y')
        ax3.set_xticks(x_positions)
        ax3.set_xticklabels(commits, rotation=45, ha='right')
        
        # Adjust layout and save
        plt.tight_layout()
        
        # Save plots
        plot_path = plot_dir / "performance_plots.png"
        plt.savefig(plot_path, dpi=300, bbox_inches='tight')
        print(f"Performance plots saved to {plot_path}")
        
        # Save individual plots as well
        # Runtime plot only
        fig1, ax = plt.subplots(1, 1, figsize=(12, 6))
        ax.errorbar(x_positions, runtimes, yerr=runtime_stds, 
                   marker='o', linewidth=2, markersize=6, capsize=4, 
                   color='#2E86C1', ecolor='#5DADE2', capthick=2)
        ax.set_title('Chess Engine Runtime per Commit', fontsize=14, fontweight='bold')
        ax.set_xlabel('Commits (Chronological Order)', fontsize=12)
        ax.set_ylabel('Runtime (ms)', fontsize=12)
        ax.grid(True, alpha=0.3)
        ax.set_xticks(x_positions)
        ax.set_xticklabels(commits, rotation=45, ha='right')
        ax.legend()
        plt.tight_layout()
        runtime_path = plot_dir / "runtime_plot.png"
        plt.savefig(runtime_path, dpi=300, bbox_inches='tight')
        plt.close(fig1)
        
        # NPS plot only
        fig2, ax = plt.subplots(1, 1, figsize=(12, 6))
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
        nps_path = plot_dir / "nps_plot.png"
        plt.savefig(nps_path, dpi=300, bbox_inches='tight')
        plt.close(fig2)
        
        print(f"Individual plots saved to {runtime_path} and {nps_path}")
        
        # Display summary statistics
        print(f"\n--- Plot Statistics ---")
        print(f"Commits plotted: {len(commits)}")
        
        # Calculate performance trend
        if len(runtimes) > 1:
            runtime_trend = runtimes[-1] - runtimes[0]  # Latest - Earliest
            nps_trend = nps_values[-1] - nps_values[0]
            
            print(f"Performance trend (latest vs earliest commit):")
            if runtime_trend < 0:
                print(f"  Runtime: {abs(runtime_trend):.1f}ms faster ({abs(runtime_trend/runtimes[0]*100):.1f}% improvement)")
            else:
                print(f"  Runtime: {runtime_trend:.1f}ms slower ({runtime_trend/runtimes[0]*100:.1f}% regression)")
                
            if nps_trend > 0:
                print(f"  NPS: +{nps_trend:,.0f} ({nps_trend/nps_values[0]*100:.1f}% improvement)")
            else:
                print(f"  NPS: {nps_trend:,.0f} ({abs(nps_trend)/nps_values[0]*100:.1f}% regression)")
        
        plt.close(fig)  # Close the main figure
    
    def print_summary(self):
        """Print a summary of benchmark results."""
        if not self.results:
            print("No results to summarize")
            return
        
        print(f"\n--- Benchmark Summary ({len(self.results)} commits) ---")
        print(f"{'Commit':<10} {'Status':<15} {'Runs':<6} {'Duration':<10} {'Mean Time':<15} {'±Std':<12} {'NPS':<15} {'Message'}")
        print("-" * 120)
        
        for result in self.results:
            commit = result["commit"]["hash"][:8]
            message = result["commit"]["message"][:25] + "..." if len(result["commit"]["message"]) > 25 else result["commit"]["message"]
            
            benchmark = result["benchmark"]
            if "error" in benchmark:
                if benchmark["error"] == "validation_failed":
                    status = "Validation Failed"
                else:
                    status = benchmark["error"].replace("_", " ").title()
                runs = str(benchmark.get('runs', '0'))
                print(f"{commit:<10} {status:<15} {runs:<6} {'N/A':<10} {'N/A':<15} {'N/A':<12} {'N/A':<15} {message}")
            else:
                runs = str(benchmark.get("runs", "1"))
                duration = f"{benchmark.get('actual_duration', 0):.1f}s" if 'actual_duration' in benchmark else "N/A"
                mean_time_ms = benchmark.get("mean_time_ms", "N/A")
                std_time_ms = benchmark.get("std_time_ms", "N/A")
                mean_nps = benchmark.get("mean_nps", "N/A")
                
                mean_str = f"{mean_time_ms:.1f}ms" if isinstance(mean_time_ms, (int, float)) else str(mean_time_ms)
                std_str = f"±{std_time_ms:.1f}ms" if isinstance(std_time_ms, (int, float)) else str(std_time_ms)
                nps_str = f"{mean_nps:,.0f}" if isinstance(mean_nps, (int, float)) else str(mean_nps)
                print(f"{commit:<10} {'Success':<15} {runs:<6} {duration:<10} {mean_str:<15} {std_str:<12} {nps_str:<15} {message}")

def main():
    """Main function to run the benchmark."""
    import argparse
    
    parser = argparse.ArgumentParser(description="Benchmark chess engine performance across git commits using hyperfine with dynamic run counts")
    parser.add_argument("--repo", "-r", default=None, help="Repository path (default: parent directory if script is in scripts/, otherwise current directory)")
    parser.add_argument("--duration", "-d", type=float, default=30.0, help="Target duration for benchmarks in seconds (default: 15.0)")
    parser.add_argument("--warmup", "-w", type=int, default=3, help="Number of warmup runs (default: 3)")
    parser.add_argument("--min-runs", type=int, default=10, help="Minimum number of runs per commit (default: 10)")
    parser.add_argument("--plots-dir", default="plots", help="Directory for saving plots (default: plots)")
    parser.add_argument("--output", "-o", default="benchmark_results.json", help="Output file for results")
    
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
    
    # Check if we're in a git repository
    if not Path(repo_path, ".git").exists():
        print(f"Error: {repo_path} does not appear to be a git repository")
        sys.exit(1)
    
    # Check if Cargo.toml exists
    if not Path(repo_path, "Cargo.toml").exists():
        print(f"Error: {repo_path} does not appear to be a Rust project (no Cargo.toml found)")
        sys.exit(1)
    
    benchmark = ChessEngineBenchmark(repo_path)
    
    # Store command line arguments for the benchmark
    benchmark.target_duration = args.duration
    benchmark.min_runs = args.min_runs
    
    try:
        benchmark.run_benchmark(args.duration, args.warmup)
        
        benchmark.print_summary()
        benchmark.save_results(args.output)
        benchmark.create_plots(args.plots_dir)
    except Exception as e:
        print(f"Benchmark failed: {e}")
        # Still try to save any results we got
        if benchmark.results:
            benchmark.save_results(args.output)
        sys.exit(1)

if __name__ == "__main__":
    main()
