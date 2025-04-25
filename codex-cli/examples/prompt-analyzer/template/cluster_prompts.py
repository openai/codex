#!/usr/bin/env python3
"""End‑to‑end pipeline for analysing a collection of text prompts.

The script performs the following steps:

1.  Read a CSV file that must contain a column named ``prompt``. If an
    ``act`` column is present it is used purely for reporting purposes.
2.  Create embeddings via the OpenAI API (``text-embedding-3-small`` by
    default).  The user can optionally provide a JSON cache path so the
    expensive embedding step is only executed for new / unseen texts.
3.  Cluster the resulting vectors either with K‑Means (automatically picking
    *k* through the silhouette score) or with DBSCAN.  Outliers are flagged
    as cluster ``-1`` when DBSCAN is selected.
4.  Ask a Chat Completion model (``gpt-4o-mini`` by default) to come up with a
    short name and description for every cluster.
5.  Write a human‑readable Markdown report (default: ``analysis.md``).
6.  Generate a couple of diagnostic plots (cluster sizes and a t‑SNE scatter
    plot) and store them in ``plots/``.

The script is intentionally opinionated yet configurable via a handful of CLI
options – run ``python cluster_prompts.py --help`` for details.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any, Sequence

import numpy as np
import pandas as pd

# External, heavy‑weight libraries are imported lazily so that users running the
# ``--help`` command do not pay the startup cost.


def parse_cli() -> argparse.Namespace:  # noqa: D401
    """Parse command‑line arguments."""

    parser = argparse.ArgumentParser(
        prog="cluster_prompts.py",
        description="Embed, cluster and analyse text prompts via the OpenAI API.",
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )

    parser.add_argument("--csv", type=Path, default=Path("prompts.csv"), help="Input CSV file.")
    parser.add_argument(
        "--cache",
        type=Path,
        default=None,
        help="Optional JSON cache for embeddings (will be created if it does not exist).",
    )
    parser.add_argument(
        "--embedding-model",
        default="text-embedding-3-small",
        help="OpenAI embedding model to use.",
    )
    parser.add_argument(
        "--chat-model",
        default="gpt-4o-mini",
        help="OpenAI chat model for cluster descriptions.",
    )

    # Clustering parameters
    parser.add_argument(
        "--cluster-method",
        choices=["kmeans", "dbscan"],
        default="kmeans",
        help="Clustering algorithm to use.",
    )
    parser.add_argument(
        "--k-max",
        type=int,
        default=10,
        help="Upper bound for k when the kmeans method is selected.",
    )
    parser.add_argument(
        "--dbscan-min-samples",
        type=int,
        default=3,
        help="min_samples parameter for DBSCAN (only relevant when dbscan is selected).",
    )

    # Output paths
    parser.add_argument(
        "--output-md", type=Path, default=Path("analysis.md"), help="Markdown report path."
    )
    parser.add_argument(
        "--plots-dir", type=Path, default=Path("plots"), help="Directory that will hold PNG plots."
    )

    return parser.parse_args()


# ---------------------------------------------------------------------------
# Embedding helpers
# ---------------------------------------------------------------------------


def _lazy_import_openai():  # noqa: D401
    """Import *openai* only when needed to keep startup lightweight."""

    try:
        import openai  # type: ignore

        return openai
    except ImportError as exc:  # pragma: no cover – we do not test missing deps.
        raise SystemExit(
            "The 'openai' package is required but not installed.\n"
            "Run 'pip install openai' and try again."
        ) from exc


def embed_texts(texts: Sequence[str], model: str, batch_size: int = 100) -> list[list[float]]:
    """Embed *texts* with OpenAI and return a list of vectors.

    Uses batching for efficiency but remains on the safe side regarding current
    OpenAI rate limits (can be adjusted by changing *batch_size*).
    """

    openai = _lazy_import_openai()
    client = openai.OpenAI()

    embeddings: list[list[float]] = []

    for batch_start in range(0, len(texts), batch_size):
        batch = texts[batch_start : batch_start + batch_size]

        response = client.embeddings.create(input=batch, model=model)
        # The API returns the vectors in the same order as the input list.
        embeddings.extend(data.embedding for data in response.data)

    return embeddings


def load_or_create_embeddings(
    prompts: pd.Series, *, cache_path: Path | None, model: str
) -> pd.DataFrame:
    """Return a *DataFrame* with one row per prompt and the embedding columns.

    * If *cache_path* is provided and exists, known embeddings are loaded from
      the JSON cache so they don't have to be re‑generated.
    * Missing embeddings are requested from the OpenAI API and subsequently
      appended to the cache.
    * The returned DataFrame has the same index as *prompts*.
    """

    cache: dict[str, list[float]] = {}
    if cache_path and cache_path.exists():
        try:
            cache = json.loads(cache_path.read_text())
        except json.JSONDecodeError:  # pragma: no cover – unlikely.
            print("⚠️  Cache file exists but is not valid JSON – ignoring.", file=sys.stderr)

    missing_mask = ~prompts.isin(cache)

    if missing_mask.any():
        texts_to_embed = prompts[missing_mask].tolist()
        print(f"Embedding {len(texts_to_embed)} new prompt(s)…", flush=True)
        new_embeddings = embed_texts(texts_to_embed, model=model)

        # Update cache (regardless of whether we persist it to disk later on).
        cache.update(dict(zip(texts_to_embed, new_embeddings)))

        if cache_path:
            cache_path.parent.mkdir(parents=True, exist_ok=True)
            cache_path.write_text(json.dumps(cache))

    # Build a consistent embeddings matrix
    vectors = prompts.map(cache.__getitem__).tolist()  # type: ignore[arg-type]
    mat = np.array(vectors, dtype=np.float32)
    return pd.DataFrame(mat, index=prompts.index)


# ---------------------------------------------------------------------------
# Clustering helpers
# ---------------------------------------------------------------------------


def _lazy_import_sklearn_cluster():
    """Lazy import helper for scikit‑learn *cluster* sub‑module."""

    # Importing scikit‑learn is slow; defer until needed.
    from sklearn.cluster import DBSCAN, KMeans  # type: ignore
    from sklearn.metrics import silhouette_score  # type: ignore
    from sklearn.preprocessing import StandardScaler  # type: ignore

    return KMeans, DBSCAN, silhouette_score, StandardScaler


def cluster_kmeans(matrix: np.ndarray, k_max: int) -> np.ndarray:
    """Auto‑select *k* (in ``[2, k_max]``) via Silhouette score and cluster."""

    KMeans, _, silhouette_score, _ = _lazy_import_sklearn_cluster()

    best_k = None
    best_score = -1.0
    best_labels: np.ndarray | None = None

    for k in range(2, k_max + 1):
        model = KMeans(n_clusters=k, random_state=42, n_init="auto")
        labels = model.fit_predict(matrix)
        try:
            score = silhouette_score(matrix, labels)
        except ValueError:
            # Occurs when a cluster ended up with 1 sample – skip.
            continue

        if score > best_score:
            best_k = k
            best_score = score
            best_labels = labels

    if best_labels is None:  # pragma: no cover – highly unlikely.
        raise RuntimeError("Unable to find a suitable number of clusters.")

    print(f"K‑Means selected k={best_k} (silhouette={best_score:.3f}).", flush=True)
    return best_labels


def cluster_dbscan(matrix: np.ndarray, min_samples: int) -> np.ndarray:
    """Cluster with DBSCAN; *eps* is estimated via the k‑distance method."""

    _, DBSCAN, _, StandardScaler = _lazy_import_sklearn_cluster()

    # Scale features – DBSCAN is sensitive to feature scale.
    scaler = StandardScaler()
    matrix_scaled = scaler.fit_transform(matrix)

    # Heuristic: use the median of the distances to the ``min_samples``‑th
    # nearest neighbour as eps. This is a commonly used rule of thumb.
    from sklearn.neighbors import NearestNeighbors  # type: ignore  # lazy import

    neigh = NearestNeighbors(n_neighbors=min_samples)
    neigh.fit(matrix_scaled)
    distances, _ = neigh.kneighbors(matrix_scaled)
    kth_distances = distances[:, -1]
    eps = float(np.percentile(kth_distances, 90))  # choose a high‑ish value.

    print(f"DBSCAN min_samples={min_samples}, eps={eps:.3f}", flush=True)
    model = DBSCAN(eps=eps, min_samples=min_samples)
    return model.fit_predict(matrix_scaled)


# ---------------------------------------------------------------------------
# Cluster labelling helpers (LLM)
# ---------------------------------------------------------------------------


def label_clusters(
    df: pd.DataFrame, labels: np.ndarray, chat_model: str, max_examples: int = 12
) -> dict[int, dict[str, str]]:
    """Generate a name & description for each cluster label via ChatGPT.

    Returns a mapping ``label -> {"name": str, "description": str}``.
    """

    openai = _lazy_import_openai()
    client = openai.OpenAI()

    out: dict[int, dict[str, str]] = {}

    for lbl in sorted(set(labels)):
        if lbl == -1:
            # Noise (DBSCAN) – skip LLM call.
            out[lbl] = {
                "name": "Noise / Outlier",
                "description": "Prompts that do not cleanly belong to any cluster.",
            }
            continue

        # Pick a handful of example prompts to send to the model.
        examples_series = df.loc[labels == lbl, "prompt"].sample(
            min(max_examples, (labels == lbl).sum()), random_state=42
        )
        examples = examples_series.tolist()

        user_content = (
            "The following text snippets are all part of the same semantic cluster.\n"
            "Please propose \n"
            "1. A very short *title* for the cluster (≤ 4 words).\n"
            "2. A concise 2–3 sentence *description* that explains the common theme.\n\n"
            "Answer **strictly** as valid JSON with the keys 'name' and 'description'.\n\n"
            "Snippets:\n"
        )
        user_content += "\n".join(f"- {t}" for t in examples)

        messages = [
            {
                "role": "system",
                "content": "You are an expert analyst, competent in summarising text clusters succinctly.",
            },
            {"role": "user", "content": user_content},
        ]

        try:
            resp = client.chat.completions.create(model=chat_model, messages=messages)
            reply = resp.choices[0].message.content.strip()

            # Extract the JSON object even if the assistant wrapped it in markdown
            # code fences or added other text.

            # Remove common markdown fences.
            reply_clean = reply.strip()
            # Take the substring between the first "{" and the last "}".
            m_start = reply_clean.find("{")
            m_end = reply_clean.rfind("}")
            if m_start == -1 or m_end == -1:
                raise ValueError("No JSON object found in model reply.")

            json_str = reply_clean[m_start : m_end + 1]
            data = json.loads(json_str)  # type: ignore[arg-type]

            out[lbl] = {
                "name": str(data.get("name", "Unnamed"))[:60],
                "description": str(data.get("description", "")).strip(),
            }
        except Exception as exc:  # pragma: no cover – network / runtime errors.
            print(f"⚠️  Failed to label cluster {lbl}: {exc}", file=sys.stderr)
            out[lbl] = {"name": f"Cluster {lbl}", "description": "<LLM call failed>"}

    return out


# ---------------------------------------------------------------------------
# Reporting helpers
# ---------------------------------------------------------------------------


def generate_markdown_report(
    df: pd.DataFrame,
    labels: np.ndarray,
    meta: dict[int, dict[str, str]],
    outputs: dict[str, Any],
    path_md: Path,
):
    """Write a self‑contained Markdown analysis to *path_md*."""

    path_md.parent.mkdir(parents=True, exist_ok=True)

    cluster_ids = sorted(set(labels))
    counts = {lbl: int((labels == lbl).sum()) for lbl in cluster_ids}

    lines: list[str] = []

    lines.append("# Prompt Clustering Report\n")
    lines.append(f"Generated by `cluster_prompts.py` – {pd.Timestamp.now()}\n")

    # High‑level stats
    total = len(labels)
    num_clusters = len(cluster_ids) - (1 if -1 in cluster_ids else 0)
    lines.append("\n## Overview\n")
    lines.append(f"* Total prompts: **{total}**")
    lines.append(f"* Clustering method: **{outputs['method']}**")
    if outputs.get("k"):
        lines.append(f"* k (K‑Means): **{outputs['k']}**")
        lines.append(f"* Silhouette score: **{outputs['silhouette']:.3f}**")
    lines.append(f"* Final clusters (excluding noise): **{num_clusters}**\n")

    # Summary table
    lines.append("\n| label | name | #prompts | description |")
    lines.append("|-------|------|---------:|-------------|")
    for lbl in cluster_ids:
        meta_lbl = meta[lbl]
        lines.append(f"| {lbl} | {meta_lbl['name']} | {counts[lbl]} | {meta_lbl['description']} |")

    # Detailed section per cluster
    for lbl in cluster_ids:
        lines.append("\n---\n")
        meta_lbl = meta[lbl]
        lines.append(f"### Cluster {lbl}: {meta_lbl['name']} ({counts[lbl]} prompts)\n")
        lines.append(f"{meta_lbl['description']}\n")

        # Show a handful of illustrative prompts.
        sample_n = min(5, counts[lbl])
        examples = df.loc[labels == lbl, "prompt"].sample(sample_n, random_state=42).tolist()
        lines.append("\nExamples:\n")
        lines.extend([f"* {t}" for t in examples])

    # Outliers / ambiguous prompts, if any.
    if -1 in cluster_ids:
        lines.append("\n---\n")
        lines.append(f"### Noise / outliers ({counts[-1]} prompts)\n")
        examples = (
            df.loc[labels == -1, "prompt"].sample(min(10, counts[-1]), random_state=42).tolist()
        )
        lines.extend([f"* {t}" for t in examples])

    # Optional ambiguous set (for kmeans)
    ambiguous = outputs.get("ambiguous", [])
    if ambiguous:
        lines.append("\n---\n")
        lines.append(f"### Potentially ambiguous prompts ({len(ambiguous)})\n")
        lines.extend([f"* {t}" for t in ambiguous])

    # Plot references
    lines.append("\n---\n")
    lines.append("## Plots\n")
    lines.append(
        "The directory `plots/` contains a bar chart of the cluster sizes and a t‑SNE scatter plot coloured by cluster.\n"
    )

    path_md.write_text("\n".join(lines))


# ---------------------------------------------------------------------------
# Plotting helpers
# ---------------------------------------------------------------------------


def create_plots(
    matrix: np.ndarray,
    labels: np.ndarray,
    for_devs: pd.Series | None,
    plots_dir: Path,
):
    """Generate cluster size and t‑SNE plots."""

    import matplotlib.pyplot as plt  # type: ignore – heavy, lazy import.
    from sklearn.manifold import TSNE  # type: ignore – heavy, lazy import.

    plots_dir.mkdir(parents=True, exist_ok=True)

    # Bar chart with cluster sizes
    unique, counts = np.unique(labels, return_counts=True)
    order = np.argsort(-counts)  # descending
    unique, counts = unique[order], counts[order]

    plt.figure(figsize=(8, 4))
    plt.bar([str(u) for u in unique], counts, color="steelblue")
    plt.xlabel("Cluster label")
    plt.ylabel("# prompts")
    plt.title("Cluster sizes")
    plt.tight_layout()
    bar_path = plots_dir / "cluster_sizes.png"
    plt.savefig(bar_path, dpi=150)
    plt.close()

    # t‑SNE scatter
    tsne = TSNE(
        n_components=2, perplexity=min(30, len(matrix) // 3), random_state=42, init="random"
    )
    xy = tsne.fit_transform(matrix)

    plt.figure(figsize=(7, 6))
    scatter = plt.scatter(xy[:, 0], xy[:, 1], c=labels, cmap="tab20", s=20, alpha=0.8)
    plt.title("t‑SNE projection")
    plt.xticks([])
    plt.yticks([])

    if for_devs is not None:
        # Overlay dev prompts as black edge markers
        dev_mask = for_devs.astype(bool).values
        plt.scatter(
            xy[dev_mask, 0],
            xy[dev_mask, 1],
            facecolors="none",
            edgecolors="black",
            linewidths=0.6,
            s=40,
            label="for_devs = TRUE",
        )
        plt.legend(loc="best")

    tsne_path = plots_dir / "tsne.png"
    plt.tight_layout()
    plt.savefig(tsne_path, dpi=150)
    plt.close()


# ---------------------------------------------------------------------------
# Main entry point
# ---------------------------------------------------------------------------


def main() -> None:  # noqa: D401
    args = parse_cli()

    # Read CSV – require a 'prompt' column.
    df = pd.read_csv(args.csv)
    if "prompt" not in df.columns:
        raise SystemExit("Input CSV must contain a 'prompt' column.")

    # Keep relevant columns only for clarity.
    df = df[[c for c in df.columns if c in {"act", "prompt", "for_devs"}]]

    # ---------------------------------------------------------------------
    # 1. Embeddings (may be cached)
    # ---------------------------------------------------------------------
    embeddings_df = load_or_create_embeddings(
        df["prompt"], cache_path=args.cache, model=args.embedding_model
    )

    # ---------------------------------------------------------------------
    # 2. Clustering
    # ---------------------------------------------------------------------
    mat = embeddings_df.values.astype(np.float32)

    if args.cluster_method == "kmeans":
        labels = cluster_kmeans(mat, k_max=args.k_max)
    else:
        labels = cluster_dbscan(mat, min_samples=args.dbscan_min_samples)

    # Identify potentially ambiguous prompts (only meaningful for kmeans).
    outputs: dict[str, Any] = {"method": args.cluster_method}
    if args.cluster_method == "kmeans":
        from sklearn.cluster import KMeans  # type: ignore – lazy

        best_k = len(set(labels))
        # Re‑fit KMeans with the chosen k to get distances.
        kmeans = KMeans(n_clusters=best_k, random_state=42, n_init="auto").fit(mat)
        outputs["k"] = best_k
        # Silhouette score (again) – not super efficient but okay.
        from sklearn.metrics import silhouette_score  # type: ignore

        outputs["silhouette"] = silhouette_score(mat, labels)

        distances = kmeans.transform(mat)
        # Ambiguous if the ratio between 1st and 2nd closest centroid < 1.1
        sorted_dist = np.sort(distances, axis=1)
        ratio = sorted_dist[:, 0] / (sorted_dist[:, 1] + 1e-9)
        ambiguous_mask = ratio > 0.9  # tunes threshold – close centroids.
        outputs["ambiguous"] = df.loc[ambiguous_mask, "prompt"].tolist()

    # ---------------------------------------------------------------------
    # 3. LLM naming / description
    # ---------------------------------------------------------------------
    meta = label_clusters(df, labels, chat_model=args.chat_model)

    # ---------------------------------------------------------------------
    # 4. Plots
    # ---------------------------------------------------------------------
    create_plots(mat, labels, df.get("for_devs"), args.plots_dir)

    # ---------------------------------------------------------------------
    # 5. Markdown report
    # ---------------------------------------------------------------------
    generate_markdown_report(df, labels, meta, outputs, path_md=args.output_md)

    print(f"✅ Done. Report written to {args.output_md} – plots in {args.plots_dir}/", flush=True)


if __name__ == "__main__":
    # Guard the main block to allow safe import elsewhere.
    main()
