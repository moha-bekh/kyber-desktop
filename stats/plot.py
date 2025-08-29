#!/usr/bin/env python3
import json
import matplotlib.pyplot as plt
import matplotlib.ticker as ticker
import numpy as np
import sys

input = "metrics.json"
if len(sys.argv) > 1:
    input = sys.argv[1]

frames = []

print(f"Opening {input}")
with open(input) as f:
    for l in f:
        j = json.loads(l)
        if j["log_type"] == "frame_metric":
            frames.append(j)

x = []
y = {
    "encoding": [],
    "encoded": [],
    "sent": [],
    "received": [],
    "decoding": [],
    "decoded": [],
    "prepared": [],
    "displayed": [],
    "skipped": [],
}

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


for f in sorted(frames, key=lambda f: f["pts"]):
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
        print(f"pts={pts} frame incomplete: {f}")

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
ax.set_title('Kyber latency')
ax.set_xlabel('pts (s)')
ax.set_ylabel('Delay (ms)')
formatter = ticker.FuncFormatter(lambda x, _: f'{x / 1_000}')
ax.xaxis.set_major_formatter(ticker.FuncFormatter(lambda x, _: f'{x / 1_000_000}'));
ax.yaxis.set_major_formatter(ticker.FuncFormatter(lambda x, _: f'{x / 1_000}'));
ax.set_xlim(0, max(x))
ax.set_ylim(0, 80000)
for i, (k, v) in enumerate(reversed(y.items())):
    ax.vlines(x, 0, v, alpha=1, color=colors[i], label=k)

ax.legend(loc='upper left')
plt.show()
