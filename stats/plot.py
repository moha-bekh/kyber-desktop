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
with open(input) as f:
    for line_num, line in enumerate(f, 1):
        stripped = line.strip()
        if not stripped:
            continue
        try:
            j = json.loads(stripped)
        except json.JSONDecodeError:
            print(f"Skipping invalid JSON on line {line_num}")
            continue
        if j["log_type"] == "frame_metric":
            source_id = j["source_id"]
            frames[source_id].append(j)

x = []
y = {
    "acquired": [],  # Reference point (always 0)
    "encoding": [],
    "encoded": [],
    "sent": [],
    "received": [],
    "decoding": [],
    "decoded": [],
    "prepared": [],
    "displayed": [],
    "skipped": [],
    "watermarking": [],
    "watermarked": [],
}

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
    median = np.median(data)
    deciles = np.percentile(data, [10, 20, 30, 40, 50, 60, 70, 80, 90])

    print(f"\tMinimum: {minimum}")
    print(f"\tMaximum: {maximum}")
    print(f"\tMedian: {median}")
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
            print(f"source_id={source_id} pts={pts} frame incomplete: {f}")


    print(f"Sample count: {len(x)}")

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
    fig = plt.figure(figsize=(18, 14))
    gs = fig.add_gridspec(5, 3, hspace=0.4, wspace=0.35)

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

    # Helper function to create distribution histograms
    def create_distribution_plot(ax, start_field, end_field, title, color):
        latency = []
        for i in range(len(x)):
            if not np.isnan(y[end_field][i]) and not np.isnan(y[start_field][i]):
                latency.append(y[end_field][i] - y[start_field][i])

        if latency:
            latency_ms = np.array(latency) / 1000
            ax.hist(latency_ms, bins=25, color=color, alpha=0.7, edgecolor='black', linewidth=0.5)
            median_val = np.median(latency_ms)
            ax.axvline(median_val, color='red', linestyle='--', linewidth=2,
                      label=f'Median: {median_val:.2f}ms')
            ax.set_xlabel('Latency (ms)', fontsize=9)
            ax.set_ylabel('Count', fontsize=9)
            ax.set_title(title, fontsize=10, fontweight='bold')
            ax.grid(True, alpha=0.3, axis='y')
            ax.legend(fontsize=8)
            ax.tick_params(labelsize=8)
        else:
            ax.text(0.5, 0.5, 'No data', ha='center', va='center', transform=ax.transAxes)
            ax.set_title(title, fontsize=10, fontweight='bold')

    # Distributions for all main metrics
    distributions = [
        (gs[2, 0], "acquired", "encoding", "Capture to Encode Start", "#FF6B6B"),
        (gs[2, 1], "acquired", "encoded", "Capture to Encoded (Server)", "#FF1493"),
        (gs[2, 2], "encoding", "encoded", "Encode Time", "#FFA07A"),
        (gs[3, 0], "sent", "received", "Network Latency", "#FFD93D"),
        (gs[3, 1], "decoding", "decoded", "Decode Time", "#4ECDC4"),
        (gs[3, 2], "decoded", "prepared", "Prepare Time", "#9B6BD8"),
        (gs[4, 0], "watermarking", "watermarked", "Watermark Time", "#2A9D8F"),
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
