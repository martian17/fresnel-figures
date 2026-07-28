# How to run
Build Fresnel first with --release.

```sh
FRESNEL_DIR="/path/to/fresnel" && \
cargo run --release -p hom-sweep-fine -- $FRESNEL_DIR && \
python hom-sweep-fine/plot.py && \
cargo run --release -p core-scaling -- $FRESNEL_DIR && \
python core-scaling/plot.py
python core-scaling/plot-vs-quisp.py
```
