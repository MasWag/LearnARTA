Artifact of "Learning Alternating Real-Time Automata"
=====================================================

This artifact accompanies the paper "Learning Alternating Real-Time Automata," submitted to QEST+FORMATS 2026. It enables reproduction of all experiments presented in the paper. The artifact can be executed on both amd64 and arm64 platforms using Docker. We also provide instructions for running the experiments without Docker.

The smoke test takes approximately 10–15 minutes to run, including setup time. The full experiments take a few hours to complete, depending on the machine. The artifact does not require network access. No special hardware or license is required to run the artifact.

The artifact includes the Docker image for amd64 and arm64 platforms, the source code, datasets, some helper scripts, and instructions for reproducing the results in the paper. The Docker image is available on [Docker Hub](https://hub.docker.com/r/maswag/learnarta) with the `latest` tag. The source code and datasets are available on [GitHub](https://github.com/MasWag/LearnARTA).

How to Install
--------------

One can load the Docker image using the following command:

```bash
./load_image.sh
```

This script loads `learnarta-amd64.tar` on `x86_64`/`amd64` hosts and
`learnarta-arm64.tar` on `aarch64`/`arm64` hosts, and tags the loaded
image as `maswag/learnarta:latest`.

Alternatively, one can run the experiments without using Docker. Here is the instructions for the installation:

1. Make sure that the following dependencies are installed:
   - Rust >= 1.88
   - Python3 (for NLStarRTA)
   - GNU time, jq, jo, and jc (for running experiments and processing the results)
   - Other common dependencies (e.g., `make`, `gcc`, `cmake`, `libclang-dev` etc.)
2. Clone the repository and move to the directory:
    ```bash
    git clone https://github.com/MasWag/LearnARTA.git --recursive
    cd LearnARTA
    ```
3. Build LearnARTA using the following command:
    ```bash
    cargo build --release
    ```

How to run the experiments
--------------------------

First, one can start a container using the following command:

```bash
docker run -it --rm maswag/learnarta:latest
```

In the container, the source code, pre-built tool, and the datasets are located in the `/LearnARTA` directory.

Then, move to the `/LearnARTA` directory with the following command:

```bash
cd /LearnARTA
```

Next, one can run the experiments using scripts under the `scripts` directory. The results of the experiments will be saved in the `logs` directory.

### For smoke test

For a smoke test, one can run the following command:

```bash
./scripts/run_small.sh
```

This takes less than 10 minutes to run and can be used to verify that the artifact is working correctly.

### Full experiments

For the full experiments, one can run the following command:

```bash
./scripts/run_all.sh
```

This takes a few hours to run, depending on the machine.

### Each experiment

Alternatively, one can run each experiment separately using the following commands:

```bash
./scripts/run.sh [learn-arta|nlstar-rta] [JSON_FILE]
```

A concrete example is:

```bash
./scripts/run.sh learn-arta baselines/NLStarRTA/test/3_2_2/3_2_2-1.json
```

How to show the results
-----------------------

Firstly, one has to generate a JSON file summarizing the results using the following command. This script takes more than a few seconds but typically less than a minute to run.

```bash
./scripts/make_summary.sh
```

Then, one can show table corresponding to Table 1 in the paper using the following command:

```bash
python ./scripts/print_group_summary_table.py ./logs/summary.json
```

The full results can be shown using the following command:

```bash
python ./scripts/print_summary_table.py ./logs/summary.json
```

These scripts can also generate a table in LaTeX format by passing `--format latex` as an argument.

How to try other examples
-------------------------

One can try other examples by making a new JSON files representing the target ARTA and passing it to LearnARTA. Some examples of the JSON files can be found in the `examples` directory. We also provide a JSON Schema for the JSON file, which can be found in `learn-arta.schema.json`.

How to build the artifact
-------------------------

We provide a script `build_image.sh` to build the Docker images for the
artifact:

```bash
./build_image.sh
```

This script builds the `linux/amd64` and `linux/arm64` images from
`artifact/Dockerfile`, stores them as `learnarta-amd64.tar` and
`learnarta-arm64.tar`, and also loads them into the local Docker image
store as `maswag/learnarta:amd64` and `maswag/learnarta:arm64`.

Logs used in the paper
----------------------

We provide the logs used in the paper in the `logs-paper` directory.

