"""Render core-scaling-vs-quisp.csv as a PGF figure for the thesis."""

import csv

import matplotlib

matplotlib.use("pgf")
import matplotlib.pyplot as plt

matplotlib.rcParams.update(
    {
        "pgf.texsystem": "pdflatex",
        "font.family": "serif",
        "text.usetex": True,
        "pgf.rcfonts": False,
        "font.size": 10,
        "axes.linewidth": 0.6,
        "figure.figsize": (4.8, 3.2),
    }
)

# single-threaded QuISP baseline, so no core-count dependence
QUISP_RATE = 36398


def main() -> None:
    cores = []
    rate = []
    with open("core-scaling-vs-quisp.csv", newline="") as f:
        for row in csv.DictReader(f):
            cores.append(int(row["cores"]))
            rate.append(float(row["events_per_second"]))

    fig, ax = plt.subplots(constrained_layout=True)
    ax.plot(
        cores,
        rate,
        color="#3b6ea5",
        linewidth=1.4,
        marker="o",
        markersize=3.5,
        label="Fresnel",
    )
    ax.axhline(
        QUISP_RATE, color="#b0413e", linewidth=1.2, linestyle="--", label="QuISP"
    )
    ax.set_yscale("log")
    ax.xaxis.set_major_locator(matplotlib.ticker.MaxNLocator(integer=True))
    ax.set_xlabel(r"Number of CPU Cores")
    ax.set_ylabel(r"Simulation events (s$^{-1}$)")
    ax.grid(True, linewidth=0.4, alpha=0.3)
    ax.spines[["top", "right"]].set_visible(False)
    ax.legend(frameon=False, loc="center right")

    fig.savefig("core-scaling-vs-quisp.pgf")
    fig.savefig("core-scaling-vs-quisp.png", dpi=300)
    print("wrote core-scaling-vs-quisp.pgf and core-scaling-vs-quisp.png")


if __name__ == "__main__":
    main()
