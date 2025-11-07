#!/usr/bin/env python3
from collections import defaultdict
import json
import matplotlib.pyplot as plt
import matplotlib.ticker as ticker
import numpy as np
import sys

input_file = "metrics.json"
all_metrics = [
        "encoding", "encoded", "sent", "received",
        "decoding", "decoded", "prepared", "displayed", "skipped",
    ]
if len(sys.argv) < 2 or sys.argv[1].startswith("--metrics="):
    print("You forgot to specify the JSON file as first argument.")
    print("Usage: plot.py <file.json> or plot.py <file.json> --metrics=metric1,metric2,...")
    sys.exit(1)

input_file = sys.argv[1]
selected_metrics = all_metrics #default value

if len(sys.argv) > 2:
    arg = sys.argv[2]

    if arg.startswith("--metrics="):
        selected_metrics = arg.split("=", 1)[1].split(",")
        error = [m for m in selected_metrics if m not in all_metrics]

        if len(error) > 0:
            print("Unkown metrics : ", error, "\nYou can display those metrics : \n", all_metrics)
            sys.exit(1)
    else:
            print("No metrics specified \nUsage: plot.py <file.json> --metrics=metric1,metric2,... \n or:    plot.py <file.json> to display all metrics")
            sys.exit(1)

frames = defaultdict(list)  # map of source_id -> list of frames

print(f"Opening {input_file}")
with open(input_file) as f:
    for l in f:
        j = json.loads(l)
        if j["log_type"] == "frame_metric":
            source_id = j["source_id"]
            frames[source_id].append(j)

x = []
y = {key: [] for key in all_metrics}

def get_timings(f):
    pts = f["pts"]

    acquired = f.get("acquired", None)
    if not acquired:
        print(f"{pts} frame incomplete: {f}")

    timings = {}

    for key in y:
        if key in f:
            timings[key] = f[key] - acquired

    return timings


def calculate_statistics(start_field, end_field):
    end_array = np.array(y[end_field])
    start_array = np.array(y[start_field])
    # Only consider items where both fields are present
    mask = (~np.isnan(end_array)) & (~np.isnan(start_array))

    data = end_array[mask] - start_array[mask]

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


for source_id, l in sorted(frames.items()):
    print(f"=== SOURCE {source_id} ===")
    for f in sorted(l, key=lambda f: f["pts"]):
        try:
            pts = f["pts"]
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

    print("Decode statistics (us)")
    calculate_statistics("decoding", "decoded")

    print("Prepare statistics (us)")
    calculate_statistics("decoded", "prepared")

    print("Encode statistics (us)")
    calculate_statistics("encoding", "encoded")

    print("Network statistics (us)")
    calculate_statistics("sent", "received")

    colors = [
        "#FF7F50",  # Coral
        "#9B59B6",  # Purple
        "#E63946",  # Red
        "#F1A7A7",  # Light red
        "#A8DADC",  # Light turquoise
        "#457B9D",  # Deep turquoise
        "#1D3557",  # Dark blue
        "#F1C40F",  # Yellow
        "#2ECC71",  # Green
    ]

    fig, ax = plt.subplots()
    ax.set_title(f'Kyber latency (source={source_id})')
    ax.set_xlabel('pts (s)')
    ax.set_ylabel('Delay (ms)')
    formatter = ticker.FuncFormatter(lambda x, _: f'{x / 1_000}')
    ax.xaxis.set_major_formatter(ticker.FuncFormatter(lambda x, _: f'{x / 1_000_000}'));
    ax.yaxis.set_major_formatter(ticker.FuncFormatter(lambda x, _: f'{x / 1_000}'));
    ax.margins(0, 0)
    ax.autoscale()
    #Plot only selected metrics
    metrics_to_plot = [(k, y[k]) for k in selected_metrics]

    for i, (k, v) in enumerate(reversed(metrics_to_plot)):
        ax.vlines(x, 0, v, alpha=1, color=colors[i], label=k)

    ax.legend(loc='upper left')
    plt.show()
