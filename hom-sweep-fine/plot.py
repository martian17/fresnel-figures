"""Render hong-ou-mandel-fine.csv as a PGF figure for the thesis."""

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


def main() -> None:
    delta_cm = []
    rate = []
    with open("hong-ou-mandel-fine.csv", newline="") as f:
        for row in csv.DictReader(f):
            delta_cm.append(float(row["delta_cm"]))
            rate.append(float(row["packets_per_second"]))

    fig, ax = plt.subplots(constrained_layout=True)
    ax.plot(delta_cm, rate, color="#3b6ea5", linewidth=1.0)
    ax.set_xlabel(r"Arm length difference $\Delta L$ (cm)")
    ax.set_ylabel(r"Detected packet rate (s$^{-1}$)")
    ax.grid(True, linewidth=0.4, alpha=0.3)
    ax.spines[["top", "right"]].set_visible(False)
    ax.set_ylim(bottom=0)

    fig.savefig("hong-ou-mandel-fine.pgf")
    fig.savefig("hong-ou-mandel-fine.png", dpi=300)
    print("wrote hong-ou-mandel-fine.pgf and hong-ou-mandel-fine.png")


if __name__ == "__main__":
    main()
