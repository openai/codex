# Prompt‑Clustering Utility

This repository contains a small utility (`cluster_prompts.py`) that embeds a
list of prompts with the OpenAI Embedding API, discovers natural groupings with
unsupervised clustering, lets ChatGPT name & describe each cluster and finally
produces a concise Markdown report plus a couple of diagnostic plots.

The default input file (`prompts.csv`) ships with the repo so you can try the
script immediately, but you can of course point it at your own file.

---

## 1. Setup

1. Install the Python dependencies (preferably inside a virtual env):

```bash
pip install pandas numpy scikit-learn matplotlib openai
```

2. Export your OpenAI API key (**required**):

```bash
export OPENAI_API_KEY="sk‑..."
```

---

## 2. Basic usage

```bash
# Minimal command – runs on prompts.csv and writes analysis.md + plots/
python cluster_prompts.py
```

This will

- create embeddings with the `text-embedding-3-small` model, 
- pick a suitable number *k* via silhouette score (K‑Means),
- ask `gpt‑4o‑mini` to label & describe each cluster,
- store the results in `analysis.md`,
- and save two plots to `plots/` (`cluster_sizes.png` and `tsne.png`).

The script prints a short success message once done.

---

## 3. Command‑line options

| flag                   | default                  | description                                                                                           |
| ---------------------- | ------------------------ | ----------------------------------------------------------------------------------------------------- |
| `--csv`                | `prompts.csv`            | path to the input CSV (must contain a `prompt` column; an `act` column is used as context if present) |
| `--cache`              | _(none)_                 | embed­ding cache path (JSON). Speeds up repeated runs – new texts are appended automatically.         |
| `--cluster-method`     | `kmeans`                 | `kmeans` (with automatic *k*) or `dbscan`                                                             |
| `--k-max`              |  `10`                    | upper bound for *k* when `kmeans` is selected                                                         |
| `--dbscan-min-samples` | `3`                      | min samples parameter for DBSCAN                                                                      |
| `--embedding-model`    | `text-embedding-3-small` | any OpenAI embedding model                                                                            |
| `--chat-model`         | `gpt-4o-mini`            | chat model used to generate cluster names / descriptions                                              |
| `--output-md`          | `analysis.md`            | where to write the Markdown report                                                                    |
| `--plots-dir`          | `plots`                  | directory for generated PNGs                                                                          |

Example with customised options:

```bash
python cluster_prompts.py \
  --csv my_prompts.csv \
  --cache .cache/embeddings.json \
  --cluster-method dbscan \
  --embedding-model text-embedding-3-large \
  --chat-model gpt-4o \
  --output-md my_analysis.md \
  --plots-dir my_plots
```

---

## 4. Interpreting the output

### analysis.md

- Overview table: cluster label, generated name, member count and description.
- Detailed section for every cluster with five representative example prompts.
- Separate lists for
  - **Noise / outliers** (label `‑1` when DBSCAN is used) and
  - **Potentially ambiguous prompts** (only with K‑Means) – these are items that
    lie almost equally close to two centroids and might belong to multiple
    groups.

### plots/cluster_sizes.png

Quick bar‑chart visualisation of how many prompts ended up in each cluster.

---

## 5. Troubleshooting

- **Rate‑limits / quota errors** – lower the number of prompts per run or switch
  to a larger quota account.
- **Authentication errors** – make sure `OPENAI_API_KEY` is exported in the
  shell where you run the script.
- **Inadequate clusters** – try the other clustering method, adjust `--k-max`
  or tune DBSCAN parameters (`eps` range is inferred, `min_samples` exposed via
  CLI).
