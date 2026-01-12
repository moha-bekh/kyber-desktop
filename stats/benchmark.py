#!/usr/bin/env python3
"""
Executes multiple runs of the client with metrics collection,
generates plots for each run, and computes median metrics across all runs.
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path
from collections import defaultdict
import numpy as np
import matplotlib.pyplot as plt
import matplotlib.ticker as ticker


def parse_args():
    """Parse command line arguments."""
    parser = argparse.ArgumentParser(
        description='Run Kyber client benchmarks with automated metrics collection'
    )
    parser.add_argument(
        '--duration',
        type=float,
        required=True,
        help='Duration in seconds for each run (e.g., 180)'
    )
    parser.add_argument(
        '--runs',
        type=int,
        required=True,
        help='Number of runs to execute (e.g., 5)'
    )
    parser.add_argument(
        '--client-path',
        type=str,
        default='./run_client.sh',
        help='Path to the Kyber client script (default: ./run_client.sh)'
    )
    parser.add_argument(
        '--output-dir',
        type=str,
        default='bench_results',
        help='Output directory for results (default: bench_results)'
    )
    parser.add_argument(
        '--server-ip',
        type=str,
        default='localhost',
        help='Server IP address (default: localhost)'
    )
    parser.add_argument(
        '--start-offset',
        type=float,
        default=0,
        help='Skip first X seconds from metrics (default: 0)'
    )
    parser.add_argument(
        '--stop-offset',
        type=float,
        default=0,
        help='Skip last X seconds from metrics (default: 0)'
    )
    return parser.parse_args()


def run_client(client_path, duration_seconds, server_ip='localhost'):
    """Launch the Kyber client with metrics enabled."""
    client_exe = Path(client_path).resolve()

    if not client_exe.exists():
        print(f"Error: Client executable not found at {client_exe}")
        return False

    print(f"Launching client: {client_exe} --metrics=true --inputs=false {server_ip}")
    print(f"   Working directory: {client_exe.parent.absolute()}")

    log_dir = client_exe.parent / 'logs'
    log_dir.mkdir(exist_ok=True)
    stdout_log = log_dir / f'client_stdout_{int(time.time())}.log'
    stderr_log = log_dir / f'client_stderr_{int(time.time())}.log'

    try:
        with open(stdout_log, 'w') as out_f, open(stderr_log, 'w') as err_f:
            process = subprocess.Popen(
                [str(client_exe), '--metrics=true', '--inputs=false', server_ip],
                cwd=client_exe.parent,
                stdout=out_f,
                stderr=err_f,
                text=True
            )

            print(f"Running for {duration_seconds} seconds...")
            print(f"   Process PID: {process.pid}")
            print(f"   Stdout log: {stdout_log}")
            print(f"   Stderr log: {stderr_log}")

            # Progress countdown
            start_time = time.time()
            end_time = start_time + duration_seconds
            update_interval = 1  # Update every second

            while time.time() < end_time:
                remaining = int(end_time - time.time())
                elapsed = int(time.time() - start_time)
                progress = (elapsed / duration_seconds) * 100

                mins_remaining = remaining // 60
                secs_remaining = remaining % 60

                print(f"\r   Progress: {progress:.1f}% | Elapsed: {elapsed}s | Remaining: {mins_remaining}m {secs_remaining}s   ", end='', flush=True)

                sleep_time = min(update_interval, end_time - time.time())
                if sleep_time > 0:
                    time.sleep(sleep_time)

            print()  # New line after progress

            print("Stopping client...")
            process.terminate()

            try:
                process.wait(timeout=10)
                print("Client terminated gracefully")
            except subprocess.TimeoutExpired:
                print("Graceful termination timed out, forcing kill...")
                process.kill()
                process.wait()
                print("Client killed")

        print("\nLast 50 lines of stdout:")
        with open(stdout_log, 'r') as f:
            lines = f.readlines()
            print(''.join(lines[-50:]))

        print("\nLast 50 lines of stderr:")
        with open(stderr_log, 'r') as f:
            lines = f.readlines()
            if lines:
                print(''.join(lines[-50:]))
            else:
                print("(no errors)")

        print(f"\nFull logs available at:")
        print(f"   {stdout_log}")
        print(f"   {stderr_log}")

        print("Waiting 2 seconds for file write...")
        time.sleep(2)

        return True

    except Exception as e:
        print(f"Error running client: {e}")
        return False


def collect_metrics(client_path, run_dir):
    """Collect the metrics.json file generated by the client."""
    client_exe = Path(client_path)
    metrics_source = client_exe.parent / 'metrics.json'

    if not metrics_source.exists():
        print(f"Error: metrics.json not found at {metrics_source}")
        return False

    run_dir = Path(run_dir)
    run_dir.mkdir(parents=True, exist_ok=True)

    metrics_dest = run_dir / 'metrics.json'
    shutil.copy2(metrics_source, metrics_dest)
    print(f"Copied metrics.json to {metrics_dest}")

    return True


def run_plot_script(metrics_file, run_dir, start_offset=0, stop_offset=0):
    """Execute the plot.py script to generate visualizations."""
    # plot.py is in the same directory as this script
    plot_script = Path(__file__).parent / 'plot.py'

    if not plot_script.exists():
        print(f"Warning: Plot script not found at {plot_script}")
        return False

    print(f"Running plot script...")
    print(f"   Script: {plot_script}")
    print(f"   Output dir: {run_dir}")

    try:
        env = os.environ.copy()
        # Pass offsets via environment variables
        env['START_OFFSET_SECONDS'] = str(start_offset)
        env['STOP_OFFSET_SECONDS'] = str(stop_offset)
        result = subprocess.run(
            [sys.executable, str(plot_script), str(metrics_file.resolve())],
            cwd=run_dir,
            env=env,
            capture_output=True,
            text=True,
            timeout=120
        )

        if result.returncode != 0:
            print(f"Plot script returned non-zero exit code: {result.returncode}")
            if result.stderr:
                print(f"Error output: {result.stderr}")
        else:
            print("Plot generated successfully")

        if result.stdout:
            print(result.stdout)

        return result.returncode == 0

    except subprocess.TimeoutExpired:
        print("Plot script timed out")
        return False
    except Exception as e:
        print(f"Error running plot script: {e}")
        return False


def load_metrics(metrics_file):
    """Load metrics from a metrics.json file."""
    metrics = []

    with open(metrics_file, 'r') as f:
        for line in f:
            try:
                metric = json.loads(line)
                metrics.append(metric)
            except json.JSONDecodeError:
                continue

    return metrics


def calculate_median_metrics(output_dir, num_runs):
    """Calculate median values across all runs."""
    print("\nCalculating median metrics across all runs...")

    output_dir = Path(output_dir)
    all_metrics = []

    for run_num in range(1, num_runs + 1):
        metrics_file = output_dir / f'run_{run_num}' / 'metrics.json'

        if not metrics_file.exists():
            print(f"Warning: Missing metrics file for run {run_num}")
            continue

        metrics = load_metrics(metrics_file)
        all_metrics.append(metrics)
        print(f"  Loaded {len(metrics)} metrics from run {run_num}")

    if not all_metrics:
        print("No metrics files found!")
        return None

    frame_metrics = defaultdict(lambda: defaultdict(list))

    for run_metrics in all_metrics:
        for metric in run_metrics:
            if metric.get('log_type') == 'frame_metric':
                source_id = metric.get('source_id')

                for field, value in metric.items():
                    if isinstance(value, (int, float)) and field not in ['pts', 'source_id']:
                        frame_metrics[source_id][field].append(value)

    median_results = {}

    for source_id, fields in frame_metrics.items():
        median_results[source_id] = {}

        for field, values in fields.items():
            if values:
                median_value = np.median(values)
                median_results[source_id][field] = {
                    'median': float(median_value),
                    'min': float(np.min(values)),
                    'max': float(np.max(values)),
                    'count': len(values)
                }

    return median_results


def generate_comparison_plot(output_dir, num_runs):
    """Generate a comparison plot showing key metrics across all runs."""
    print("\nGenerating comparison plot across all runs...")

    output_dir = Path(output_dir)

    metrics_to_compare = [
        ("acquired", "encoding", "Capture to Encode", "#FF6B6B"),
        ("acquired", "encoded", "Capture to Encoded", "#FF1493"),
        ("encoding", "encoded", "Encode Time", "#FFA07A"),
        ("sent", "received", "Network Latency", "#FFD93D"),
        ("decoding", "decoded", "Decode Time", "#4ECDC4"),
        ("decoded", "prepared", "Prepare Time", "#9B6BD8"),
        ("watermarking", "watermarked", "Watermark Time", "#2A9D8F"),
    ]

    runs_data = defaultdict(lambda: defaultdict(list))

    for run_num in range(1, num_runs + 1):
        metrics_file = output_dir / f'run_{run_num}' / 'metrics.json'

        if not metrics_file.exists():
            print(f"Warning: Missing metrics file for run {run_num}")
            continue

        metrics = load_metrics(metrics_file)

        for metric in metrics:
            if metric.get('log_type') == 'frame_metric':
                acquired = metric.get('acquired')
                if not acquired:
                    continue

                for start_field, end_field, label, _ in metrics_to_compare:
                    if start_field in metric and end_field in metric:
                        if start_field == "acquired":
                            start_val = 0
                        else:
                            start_val = metric[start_field] - acquired

                        end_val = metric[end_field] - acquired
                        duration = end_val - start_val
                        runs_data[label][run_num].append(duration)

    if not runs_data:
        print("ERROR: No data to plot")
        return

    fig, axes = plt.subplots(3, 3, figsize=(20, 16))
    fig.suptitle('Kyber Benchmark Comparison Across All Runs', fontsize=16, fontweight='bold')

    axes_flat = axes.flatten()

    for idx, (start_field, end_field, label, color) in enumerate(metrics_to_compare):
        if idx >= len(axes_flat):
            break

        ax = axes_flat[idx]

        if label not in runs_data or not runs_data[label]:
            ax.text(0.5, 0.5, f'No data for {label}',
                   ha='center', va='center', transform=ax.transAxes)
            ax.set_title(label, fontweight='bold')
            continue

        data_to_plot = []
        run_labels = []

        for run_num in range(1, num_runs + 1):
            if run_num in runs_data[label] and runs_data[label][run_num]:
                data_ms = np.array(runs_data[label][run_num]) / 1000
                data_to_plot.append(data_ms)
                run_labels.append(f'Run {run_num}')

        if data_to_plot:
            bp = ax.boxplot(data_to_plot, tick_labels=run_labels, patch_artist=True)

            for patch in bp['boxes']:
                patch.set_facecolor(color)
                patch.set_alpha(0.7)

            medians = [np.median(d) for d in data_to_plot]
            ax.plot(range(1, len(medians) + 1), medians,
                   'r--', linewidth=2, marker='o', label='Median')

            ax.set_xlabel('Run', fontsize=10)
            ax.set_ylabel('Latency (ms)', fontsize=10)
            ax.set_title(label, fontsize=12, fontweight='bold')
            ax.grid(True, alpha=0.3, axis='y')
            ax.legend(fontsize=8)

            if num_runs > 5:
                ax.tick_params(axis='x', rotation=45)

    for idx in range(len(metrics_to_compare), len(axes_flat)):
        axes_flat[idx].axis('off')

    plt.tight_layout()

    comparison_file = output_dir / 'comparison_plot.png'
    plt.savefig(comparison_file, dpi=300, bbox_inches='tight')
    print(f"Comparison plot saved to {comparison_file}")

    plt.close()


def main():
    """Main execution function."""
    args = parse_args()

    print("=" * 70)
    print("Kyber Benchmark Automation")
    print("=" * 70)
    print(f"Configuration:")
    print(f"  Duration per run: {args.duration} seconds")
    print(f"  Number of runs: {args.runs}")
    print(f"  Client path: {args.client_path}")
    print(f"  Server IP: {args.server_ip}")
    print(f"  Output directory: {args.output_dir}")
    print(f"  Start offset: {args.start_offset} seconds")
    print(f"  Stop offset: {args.stop_offset} seconds")
    print("=" * 70)

    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    successful_runs = 0

    for run_num in range(1, args.runs + 1):
        print(f"\n{'=' * 70}")
        print(f"Run {run_num}/{args.runs}")
        print(f"{'=' * 70}")

        run_dir = output_dir / f'run_{run_num}'

        if not run_client(args.client_path, args.duration, args.server_ip):
            print(f"Run {run_num} failed during client execution")
            continue

        if not collect_metrics(args.client_path, run_dir):
            print(f"Run {run_num} failed during metrics collection")
            continue

        metrics_file = run_dir / 'metrics.json'
        run_plot_script(metrics_file, run_dir, args.start_offset, args.stop_offset)

        successful_runs += 1
        print(f"Run {run_num} completed successfully")

    print(f"\n{'=' * 70}")
    print("Final Analysis")
    print(f"{'=' * 70}")
    print(f"Successful runs: {successful_runs}/{args.runs}")

    median_metrics = calculate_median_metrics(output_dir, args.runs)

    if median_metrics:
        median_file = output_dir / 'median_metrics.json'
        with open(median_file, 'w') as f:
            json.dump(median_metrics, f, indent=2)

        print(f"Median metrics saved to {median_file}")

        print("\nSummary:")
        for source_id, fields in median_metrics.items():
            print(f"\n  Source ID: {source_id}")
            for field, stats in sorted(fields.items()):
                print(f"    {field}:")
                print(f"      Median: {stats['median']:.2f}")
                print(f"      Range: [{stats['min']:.2f}, {stats['max']:.2f}]")
                print(f"      Samples: {stats['count']}")

    generate_comparison_plot(output_dir, args.runs)

    print(f"\n{'=' * 70}")
    print("Benchmark completed!")
    print(f"{'=' * 70}")


if __name__ == '__main__':
    main()
