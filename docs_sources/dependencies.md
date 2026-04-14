# Dependencies
--------------

## Python & OS

As of the moment, Backtide supports the following Python versions:

* [Python 3.11](https://www.python.org/downloads/release/python-3110/)
* [Python 3.12](https://www.python.org/downloads/release/python-3120/)
* [Python 3.13](https://www.python.org/downloads/release/python-3130/)
* [Python 3.14](https://www.python.org/downloads/release/python-3140/)

And operating systems:

 * Linux (Ubuntu, Fedora, etc...)
 * Windows 8.1+
 * macOS (not tested)

<br><br>


## Python packages

### Required

* **[click](https://click.palletsprojects.com/)** (>=8.3.1)
* **[pandas](https://pandas.pydata.org/)** (>=2.3.3)
* **[pyyaml](https://pyyaml.org/)** (>=6.0.3)
* **[streamlit](https://streamlit.io/)** (>=1.55.0)
* **[streamlit-code-editor](https://github.com/bouzidanas/streamlit-code-editor)** (>=0.1.22)


### Optional

* **[polars](https://github.com/pola-rs/polars)** (>=1.0)

### Development

The development dependencies are not installed with the package, and are not
required for any of its functionalities. These libraries are only necessary to
[contribute][contributing] to the project. Install them running `uv sync --all-groups`.

**Dev**

* **[tox](https://tox.wiki/)** (>=4.50.3)
* **[tox-uv](https://github.com/tox-dev/tox-uv)** (>=1.33.4)

**Linting**

* **[pre-commit](https://pre-commit.com/)** (>=4.5.1)
* **[pre-commit-uv](https://github.com/tox-dev/pre-commit-uv)** (>=4.2.1)
* **[ruff](https://docs.astral.sh/ruff/)** (>=0.15.7)
* **[ty](https://github.com/astral-sh/ty)** (>=0.0.25)

**Testing**

* **[pytest](https://docs.pytest.org/en/latest/)** (>=8.1.1)
* **[pytest-cov](https://pytest-cov.readthedocs.io/en/latest/)** (>=4.1.0)
* **[pytest-mock](https://github.com/pytest-dev/pytest-mock/)** (>=3.12.0)
* **[pytest-xdist](https://github.com/pytest-dev/pytest-xdist)** (>=3.5.0)

**Documentation**

* **[kaleido](https://github.com/plotly/Kaleido)** (>=1.2.0)
* **[mike](https://github.com/jimporter/mike)** (>=2.1.4)
* **[mkdocs](https://www.mkdocs.org/)** (>=1.6.1)
* **[mkdocs-autorefs](https://mkdocstrings.github.io/autorefs/)** (>=1.4.4)
* **[mkdocs-material](https://squidfunk.github.io/mkdocs-material/)** (>=9.7.6)
* **[mkdocs-material-extensions](https://pypi.org/project/mkdocs-material-extensions/)** (>=1.3.1)
* **[mkdocs-simple-hooks](https://github.com/aklajnert/mkdocs-simple-hooks)** (>=0.1.5)
* **[pymdown-extensions](https://github.com/facelessuser/pymdown-extensions)** (>=10.21)
* **[pyyaml](https://pyyaml.org/)** (>=6.0.3)
* **[regex](https://github.com/mrabarnett/mrab-regex)** (>=2026.2.28)

<br><br>


## Rust crates

### Runtime

* **[async-trait](https://crates.io/crates/async-trait)** (0.1.89)
* **[chrono](https://crates.io/crates/chrono)** (0.4.44)
* **[duckdb](https://crates.io/crates/duckdb)** (1.10501.0)
* **[futures](https://crates.io/crates/futures)** (0.3.32)
* **[indexmap](https://crates.io/crates/indexmap)** (2.13.1)
* **[moka](https://crates.io/crates/moka)** (0.12.15)
* **[pyo3](https://crates.io/crates/pyo3)** (0.28.3)
* **[pythonize](https://crates.io/crates/pythonize)** (0.28.0)
* **[reqwest](https://crates.io/crates/reqwest)** (0.13.2)
* **[serde](https://crates.io/crates/serde)** (1.0.228)
* **[serde_json](https://crates.io/crates/serde_json)** (1.0.149)
* **[serde_yml](https://crates.io/crates/serde_yml)** (0.0.12)
* **[serde_with](https://crates.io/crates/serde_with)** (3.18.0)
* **[strum](https://crates.io/crates/strum)** (0.28.0)
* **[thiserror](https://crates.io/crates/thiserror)** (2.0.18)
* **[tokio](https://crates.io/crates/tokio)** (1.51.1)
* **[toml](https://crates.io/crates/toml)** (1.1.0)
* **[tracing](https://crates.io/crates/tracing)** (0.1.44)
* **[tracing-subscriber](https://crates.io/crates/tracing-subscriber)** (0.3.23)

### Dev

* **[criterion](https://crates.io/crates/criterion)** (0.8.2)
* **[tempfile](https://crates.io/crates/tempfile)** (3.27.0)
