#!/usr/bin/env python3
from collections import defaultdict
import json
import matplotlib.pyplot as plt
import matplotlib.ticker as ticker
import numpy as np
import sys
import os

# Configurable offsets (in seconds)
START_OFFSET_SECONDS = float(os.environ.get("START_OFFSET_SECONDS", 0))
STOP_OFFSET_SECONDS = float(os.environ.get("STOP_OFFSET_SECONDS", 0))

input = "metrics.json"
if len(sys.argv) > 1:
    input = sys.argv[1]
if len(sys.argv) > 2:
    START_OFFSET_SECONDS = float(sys.argv[2])
if len(sys.argv) > 3:
    STOP_OFFSET_SECONDS = float(sys.argv[3])

# Extract directory name for output file naming
input_dir = os.path.dirname(input)
if input_dir:
    input_basename = os.path.basename(input_dir)
else:
    input_basename = "default"

print(f"Time window: skip first {START_OFFSET_SECONDS}s and last {STOP_OFFSET_SECONDS}s")

frames = defaultdict(list)  # map of source_id -> list of frames

print(f"Opening {input}")
invalid_json_count = 0
with open(input) as f:
    for line_num, line in enumerate(f, 1):
        stripped = line.strip()
        if not stripped:
            continue
        try:
            j = json.loads(stripped)
        except json.JSONDecodeError:
            print(f"Skipping invalid JSON on line {line_num}")
            invalid_json_count += 1
            continue
        if j["log_type"] == "frame_metric":
            source_id = j["source_id"]
            frames[source_id].append(j)

print(f"Invalid JSON lines: {invalid_json_count}")

# x and y are (re)initialized per source_id inside the main loop below, so data from
# one source never bleeds into another source's stats/plots.
METRIC_KEYS = [
    "acquired",  # Reference point (always 0)
    "encoding",
    "encoded",
    "sent",
    "received",
    "decoding",
    "decoded",
    "prepared",
    "displayed",
    "skipped",
    "watermarking",
    "watermarked",
]
x = []
y = {key: [] for key in METRIC_KEYS}

def get_timings(f):
    pts = f["pts"]

    acquired = f.get("acquired", None)
    if not acquired:
        print(f"{pts} frame incomplete: {f}")

    timings = {}
    timings["acquired"] = 0  # Reference point

    for key in y:
        if key in f and key != "acquired":
            timings[key] = f[key] - acquired

    return timings


def calculate_statistics(start_field, end_field):
    end_array = np.array(y[end_field])
    start_array = np.array(y[start_field])
    # Only consider items where both fields are present
    mask = (~np.isnan(end_array)) & (~np.isnan(start_array))

    data = end_array[mask] - start_array[mask]

    if len(data) == 0:
        print(f"\tNo data available (fields '{start_field}' and/or '{end_field}' not present in frames)")
        return

    minimum = np.min(data)
    maximum = np.max(data)
    mean = np.mean(data)
    std = np.std(data)
    median = np.median(data)
    p90, p99 = np.percentile(data, [90, 99])
    deciles = np.percentile(data, [10, 20, 30, 40, 50, 60, 70, 80, 90])

    print(f"\tMinimum: {minimum}")
    print(f"\tMaximum: {maximum}")
    print(f"\tMean: {mean:.1f}")
    print(f"\tStd Dev: {std:.1f}")
    print(f"\tP50 (Median): {median}")
    print(f"\tP90: {p90}")
    print(f"\tP99: {p99}")
    print("\tDeciles:")
    for i, decile in enumerate(deciles, 1):
        print(f"\t\tD{i} ({i}0th percentile): {decile}")


class ZoomPan:
    """Class to manage interactive zoom and pan"""
    def __init__(self):
        self.press = None
        self.cur_xlim = None
        self.cur_ylim = None
        self.x0 = None
        self.y0 = None
        self.x1 = None
        self.y1 = None
        self.xpress = None
        self.ypress = None

    def zoom_factory(self, ax, base_scale=1.2):
        """Enable zoom with mouse wheel"""
        def zoom(event):
            if event.inaxes != ax:
                return

            cur_xlim = ax.get_xlim()
            cur_ylim = ax.get_ylim()
            xdata = event.xdata
            ydata = event.ydata

            if event.button == 'up':
                # Zoom in
                scale_factor = 1 / base_scale
            elif event.button == 'down':
                # Zoom out
                scale_factor = base_scale
            else:
                return

            new_width = (cur_xlim[1] - cur_xlim[0]) * scale_factor
            new_height = (cur_ylim[1] - cur_ylim[0]) * scale_factor

            relx = (cur_xlim[1] - xdata) / (cur_xlim[1] - cur_xlim[0])
            rely = (cur_ylim[1] - ydata) / (cur_ylim[1] - cur_ylim[0])

            ax.set_xlim([xdata - new_width * (1 - relx), xdata + new_width * relx])
            ax.set_ylim([ydata - new_height * (1 - rely), ydata + new_height * rely])
            ax.figure.canvas.draw()

        fig = ax.get_figure()
        fig.canvas.mpl_connect('scroll_event', zoom)
        return zoom

    def pan_factory(self, ax):
        """Enable pan with click + drag"""
        def onPress(event):
            if event.inaxes != ax:
                return
            self.cur_xlim = ax.get_xlim()
            self.cur_ylim = ax.get_ylim()
            self.press = self.x0, self.y0, event.xdata, event.ydata
            self.x0, self.y0, self.xpress, self.ypress = self.press

        def onRelease(event):
            self.press = None
            ax.figure.canvas.draw()

        def onMotion(event):
            if self.press is None:
                return
            if event.inaxes != ax:
                return
            dx = event.xdata - self.xpress
            dy = event.ydata - self.ypress
            self.cur_xlim -= dx
            self.cur_ylim -= dy
            ax.set_xlim(self.cur_xlim)
            ax.set_ylim(self.cur_ylim)
            ax.figure.canvas.draw()

        fig = ax.get_figure()
        fig.canvas.mpl_connect('button_press_event', onPress)
        fig.canvas.mpl_connect('button_release_event', onRelease)
        fig.canvas.mpl_connect('motion_notify_event', onMotion)
        return onMotion


for source_id, l in sorted(frames.items()):
    print(f"=== SOURCE {source_id} ===")

    # Reset per-source accumulators so one source's frames never leak into another's stats/plots
    x = []
    y = {key: [] for key in METRIC_KEYS}

    # Find min and max timestamps for filtering
    all_pts = [f["pts"] for f in l if "pts" in f]
    if not all_pts:
        continue

    min_pts = min(all_pts)
    max_pts = max(all_pts)

    # Convert seconds to microseconds
    start_offset = int(START_OFFSET_SECONDS * 1_000_000)
    stop_offset = int(STOP_OFFSET_SECONDS * 1_000_000)

    filter_start = min_pts + start_offset
    filter_end = max_pts - stop_offset

    total_duration = (max_pts - min_pts) / 1_000_000
    filtered_duration = (filter_end - filter_start) / 1_000_000

    print(f"Total recording duration: {total_duration:.1f}s")
    print(f"Filtering window: {START_OFFSET_SECONDS}s to {total_duration - STOP_OFFSET_SECONDS:.1f}s")
    print(f"Filtered duration: {filtered_duration:.1f}s ({filter_start/1_000_000:.1f}s to {filter_end/1_000_000:.1f}s)")

    incomplete_count = 0
    for f in sorted(l, key=lambda f: f["pts"]):
        try:
            pts = f["pts"]

            # Apply time filter
            if pts < filter_start or pts > filter_end:
                continue

            timings = get_timings(f)
            if timings is None:
                continue

            x.append(pts)
            for k, v in y.items():
                if k in timings:
                    v.append(timings[k])
                else:
                    v.append(np.nan) # missing value
        except:
            incomplete_count += 1
            print(f"source_id={source_id} pts={pts} frame incomplete: {f}")


    print(f"Sample count: {len(x)}")

    print("Data quality")
    print(f"\tValid frames:      {len(x)}")
    print(f"\tIncomplete frames: {incomplete_count}")

    print("Frame rate")
    if len(x) >= 2:
        intervals_us = np.diff(np.array(x, dtype=np.float64))
        intervals_us = intervals_us[intervals_us > 0]  # guard against duplicate/out-of-order pts
        if len(intervals_us) > 0:
            fps_values = 1_000_000 / intervals_us
            interval_p50, interval_p90, interval_p99 = np.percentile(intervals_us, [50, 90, 99])
            print(f"\tAverage: {np.mean(fps_values):.2f} FPS")
            print(f"\tMin:     {np.min(fps_values):.2f} FPS")
            print(f"\tMax:     {np.max(fps_values):.2f} FPS")
            print("\tFrame interval:")
            print(f"\t\tp50: {interval_p50 / 1000:.2f} ms")
            print(f"\t\tp90: {interval_p90 / 1000:.2f} ms")
            print(f"\t\tp99: {interval_p99 / 1000:.2f} ms")
        else:
            print("\tNo valid frame intervals available")
    else:
        print("\tNot enough frames to compute frame rate")

    print("Capture to Displayed statistics (us) - Total pipeline latency")
    calculate_statistics("acquired", "displayed")

    print("Capture to Skip statistics (us) - Latency before a dropped frame")
    calculate_statistics("acquired", "skipped")

    print("Capture to Encode Start statistics (us)")
    calculate_statistics("acquired", "encoding")

    print("Capture to Encoded statistics (us) - Server processing overview")
    calculate_statistics("acquired", "encoded")

    print("Decode statistics (us)")
    calculate_statistics("decoding", "decoded")

    print("Prepare statistics (us)")
    calculate_statistics("decoded", "prepared")

    print("Encode statistics (us)")
    calculate_statistics("encoding", "encoded")

    print("Network statistics (us)")
    calculate_statistics("sent", "received")

    print("Watermark statistics (us)")
    calculate_statistics("watermarking", "watermarked")

    print("Pipeline breakdown (share of mean Capture-to-Displayed latency)")
    total_start = np.array(y["acquired"])
    total_end = np.array(y["displayed"])
    total_mask = (~np.isnan(total_start)) & (~np.isnan(total_end))
    if total_mask.any():
        total_mean = np.mean(total_end[total_mask] - total_start[total_mask])
        stage_fields = [
            ("Capture to Encode Start", "acquired", "encoding"),
            ("Encode", "encoding", "encoded"),
            ("Network", "sent", "received"),
            ("Decode", "decoding", "decoded"),
            ("Prepare", "decoded", "prepared"),
            ("Watermark", "watermarking", "watermarked"),
        ]
        for name, start_field, end_field in stage_fields:
            s = np.array(y[start_field])
            e = np.array(y[end_field])
            mask = (~np.isnan(s)) & (~np.isnan(e))
            if not mask.any():
                continue
            stage_mean = np.mean(e[mask] - s[mask])
            pct = 100 * stage_mean / total_mean if total_mean > 0 else 0
            print(f"\t{name:<24} {stage_mean / 1000:>7.2f} ms  {pct:>5.1f}%")
    else:
        print("\tNo data available (fields 'acquired' and/or 'displayed' not present in frames)")

    colors = {
        "acquired": "#808080",      # Gray - reference point (baseline)
        "encoding": "#FF6B6B",      # Red - start of pipeline
        "encoded": "#FFA07A",       # Light salmon
        "sent": "#FF1493",          # Deep pink - server processing complete
        "received": "#6BCF7F",      # Green - network end
        "decoding": "#4ECDC4",      # Turquoise - processing
        "decoded": "#45B7D1",       # Sky blue
        "prepared": "#9B6BD8",      # Purple
        "displayed": "#C77DFF",     # Light purple - end of pipeline
        "skipped": "#E63946",       # Bright red - errors
        "watermarking": "#2A9D8F",  # Teal
        "watermarked": "#264653",   # Dark teal
    }

    # Create figure with multiple subplots
    fig = plt.figure(figsize=(18, 17))
    gs = fig.add_gridspec(6, 3, hspace=0.45, wspace=0.35)

    # Main timeline plot (spans full width at top)
    ax1 = fig.add_subplot(gs[0:2, :])
    ax1.set_title(f'Kyber Frame Pipeline Latency (source={source_id})\n[Mouse Wheel: Zoom | Left Click + Drag: Pan | R: Reset]',
                  fontsize=14, fontweight='bold')
    ax1.set_xlabel('Time (s)', fontsize=12)
    ax1.set_ylabel('Latency from Acquisition (ms)', fontsize=12)
    ax1.xaxis.set_major_formatter(ticker.FuncFormatter(lambda x, _: f'{x / 1_000_000:.1f}'))
    ax1.yaxis.set_major_formatter(ticker.FuncFormatter(lambda x, _: f'{x / 1_000:.1f}'))
    ax1.grid(True, alpha=0.3, linestyle='--')
    ax1.margins(0.01, 0.05)

    # Frame budget reference lines (60 FPS / 120 FPS), drawn under the data
    for budget_us, budget_label in [(16_667, "60 FPS budget (16.67 ms)"), (8_333, "120 FPS budget (8.33 ms)")]:
        ax1.axhline(budget_us, color="0.3", linestyle=":", linewidth=1.2, alpha=0.6, zorder=0)
        ax1.text(1.005, budget_us, budget_label, transform=ax1.get_yaxis_transform(),
                 va='center', ha='left', fontsize=8, color="0.3", alpha=0.9, clip_on=False)

    # Plot each stage with improved styling
    for k, v in reversed(list(y.items())):
        valid_data = [(x[i], v[i]) for i in range(len(x)) if not np.isnan(v[i])]
        if valid_data:
            x_vals, y_vals = zip(*valid_data)
            ax1.vlines(x_vals, 0, y_vals, alpha=0.9, linewidth=0.8,
                      color=colors.get(k, "#95A5A6"), label=k)

    handles, labels = ax1.get_legend_handles_labels()
    ax1.legend(handles[::-1], labels[::-1], loc='upper left', framealpha=0.9, fontsize=10)

    # Enable zoom and pan for the main plot
    zp = ZoomPan()
    zp.zoom_factory(ax1, base_scale=1.3)
    zp.pan_factory(ax1)

    # Save initial limits for reset
    ax1_initial_xlim = ax1.get_xlim()
    ax1_initial_ylim = ax1.get_ylim()

    # Reset function
    def reset_view(event):
        if event.key == 'r':
            ax1.set_xlim(ax1_initial_xlim)
            ax1.set_ylim(ax1_initial_ylim)
            ax1.figure.canvas.draw()

    fig.canvas.mpl_connect('key_press_event', reset_view)

    # Latency-over-time plot: per-stage *durations* (not cumulative from acquisition),
    # overlaid so temporal patterns (periodic spikes, drift, step changes) stand out.
    ax_time = fig.add_subplot(gs[2, :])
    ax_time.set_title('Latency Over Time (per-stage duration)', fontsize=12, fontweight='bold')
    ax_time.set_xlabel('Time (s)', fontsize=10)
    ax_time.set_ylabel('Duration (ms)', fontsize=10)
    ax_time.xaxis.set_major_formatter(ticker.FuncFormatter(lambda x, _: f'{x / 1_000_000:.1f}'))
    ax_time.grid(True, alpha=0.3, linestyle='--')

    time_series = [
        ("Encode", "encoding", "encoded", "#FF6B6B"),
        ("Network", "sent", "received", "#FFD93D"),
        ("Decode", "decoding", "decoded", "#4ECDC4"),
        ("Total", "acquired", "displayed", "#9B6BD8"),
    ]
    x_arr = np.array(x, dtype=np.float64)
    # Draw largest-duration series first (Total) and smallest last, same trick as the
    # main plot, so short bars aren't buried under taller ones sharing the same x.
    for name, start_field, end_field, color in reversed(time_series):
        s = np.array(y[start_field])
        e = np.array(y[end_field])
        mask = (~np.isnan(s)) & (~np.isnan(e))
        if not mask.any():
            continue
        ax_time.vlines(x_arr[mask], 0, (e[mask] - s[mask]) / 1000, alpha=0.8, linewidth=0.8,
                       color=color, label=name)

    handles, labels = ax_time.get_legend_handles_labels()
    ax_time.legend(handles[::-1], labels[::-1], loc='upper left', framealpha=0.9, fontsize=9)

    zp_time = ZoomPan()
    zp_time.zoom_factory(ax_time, base_scale=1.3)
    zp_time.pan_factory(ax_time)

    # Helper function to create distribution histograms
    def create_distribution_plot(ax, start_field, end_field, title, color):
        latency = []
        for i in range(len(x)):
            if not np.isnan(y[end_field][i]) and not np.isnan(y[start_field][i]):
                latency.append(y[end_field][i] - y[start_field][i])

        if latency:
            latency_ms = np.array(latency) / 1000
            ax.hist(latency_ms, bins=25, color=color, alpha=0.7, edgecolor='black', linewidth=0.5)
            p50, p90, p99 = np.percentile(latency_ms, [50, 90, 99])
            ax.axvline(p50, color='red', linestyle='--', linewidth=2, label=f'P50: {p50:.2f}ms')
            ax.axvline(p90, color='darkorange', linestyle='-.', linewidth=1.5, label=f'P90: {p90:.2f}ms')
            ax.axvline(p99, color='black', linestyle=':', linewidth=1.5, label=f'P99: {p99:.2f}ms')
            ax.set_xlabel('Latency (ms)', fontsize=9)
            ax.set_ylabel('Count', fontsize=9)
            ax.set_title(title, fontsize=10, fontweight='bold')
            ax.grid(True, alpha=0.3, axis='y')
            ax.legend(fontsize=7)
            ax.tick_params(labelsize=8)
        else:
            ax.text(0.5, 0.5, 'No data', ha='center', va='center', transform=ax.transAxes)
            ax.set_title(title, fontsize=10, fontweight='bold')

    # Distributions for all main metrics
    distributions = [
        (gs[3, 0], "acquired", "encoding", "Capture to Encode Start", "#FF6B6B"),
        (gs[3, 1], "acquired", "encoded", "Capture to Encoded (Server)", "#FF1493"),
        (gs[3, 2], "encoding", "encoded", "Encode Time", "#FFA07A"),
        (gs[4, 0], "sent", "received", "Network Latency", "#FFD93D"),
        (gs[4, 1], "decoding", "decoded", "Decode Time", "#4ECDC4"),
        (gs[4, 2], "decoded", "prepared", "Prepare Time", "#9B6BD8"),
        (gs[5, 0], "watermarking", "watermarked", "Watermark Time", "#2A9D8F"),
        (gs[5, 1], "acquired", "displayed", "Capture to Displayed (Total)", "#C77DFF"),
        (gs[5, 2], "acquired", "skipped", "Capture to Skip (Dropped Frames)", "#E63946"),
    ]

    for grid_pos, start_field, end_field, title, color in distributions:
        ax = fig.add_subplot(grid_pos)
        create_distribution_plot(ax, start_field, end_field, title, color)
        # Add zoom/pan to histograms too
        zp_hist = ZoomPan()
        zp_hist.zoom_factory(ax, base_scale=1.2)
        zp_hist.pan_factory(ax)

    plt.suptitle(f'Kyber Performance Metrics Analysis', fontsize=16, fontweight='bold', y=0.995)

    # Save the figure
    output_file = f"kyber_metrics_{input_basename}_source_{source_id}.png"
    plt.savefig(output_file, dpi=300, bbox_inches='tight')
    print(f"\nPlot saved to: {output_file}")

    #plt.show()
