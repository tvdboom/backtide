# Dependencies
--------------

## Python & OS

As of the moment, backtide supports the following Python versions:

* [Python 3.11](https://www.python.org/downloads/release/python-3110/)
* [Python 3.12](https://www.python.org/downloads/release/python-3120/)
* [Python 3.13](https://www.python.org/downloads/release/python-3130/)
* [Python 3.14](https://www.python.org/downloads/release/python-3140/)

And operating systems:

 * Linux (Ubuntu, Fedora, etc...)
 * Windows 8.1+
 * macOS (not tested)

<br><br>


## Packages

### Required

Backtide is built on top of several existing Python libraries. These
packages are necessary for its correct functioning.

* **[beartype](https://beartype.readthedocs.io/en/latest/)** (>=0.18.5)
* **[category-encoders](https://contrib.scikit-learn.org/categorical-encoding/index.html)** (>=2.6.3)
* **[dill](https://pypi.org/project/dill/)** (>=0.3.6)
* **[featuretools](https://www.featuretools.com/)** (>=1.28.0)
* **[gplearn](https://gplearn.readthedocs.io/en/stable/index.html)** (>=0.4.2)
* **[imbalanced-learn](https://imbalanced-learn.readthedocs.io/en/stable/api.html)** (>=0.12.3)
* **[ipython](https://ipython.readthedocs.io/en/stable/)** (>=8.9.0)
* **[ipywidgets](https://pypi.org/project/ipywidgets/)** (>=8.1.1)
* **[joblib](https://joblib.readthedocs.io/en/latest/)** (>=1.3.1)
* **[matplotlib](https://matplotlib.org/)** (>=3.7.2)
* **[mlflow](https://mlflow.org/)** (>=2.10.2)
* **[nltk](https://www.nltk.org/)** (>=3.8.1)
* **[numpy](https://numpy.org/)** (>=1.23.0)
* **[optuna](https://optuna.org/)** (>=3.6.0)
* **[pandas](https://pandas.pydata.org/)** (>=2.2.2)
* **[plotly](https://plotly.com/python/)** (>=5.18.0)
* **[scikit-learn](https://scikit-learn.org/stable/)** (>=1.5.0)
* **[scipy](https://www.scipy.org/)** (>=1.10.1)
* **[shap](https://github.com/slundberg/shap/)** (>=0.43.0)
* **[statsmodels](https://www.statsmodels.org/stable/index.html)** (>=0.14.1)
* **[zoofs](https://jaswinder9051998.github.io/zoofs/)** (>=0.1.26)


### Development

The development dependencies are not installed with the package, and are
not required for any of its functionalities. These libraries are only
necessary to [contribute][contributing] to the project. Install them
running `pdm install --dev` (remember to install [pdm](https://pdm-project.org/latest/) first with
`pip install -U pdm`).

**Linting**

* **[pre-commit](https://pre-commit.com/)** (>=3.6.2)

**Testing**

* **[nbmake](https://github.com/treebeardtech/nbmake)** (>=1.5.3)
* **[pytest](https://docs.pytest.org/en/latest/)** (>=8.1.1)
* **[pytest-cov](https://pytest-cov.readthedocs.io/en/latest/)** (>=4.1.0)
* **[pytest-mock](https://github.com/pytest-dev/pytest-mock/)** (>=3.12.0)
* **[pytest-xdist](https://github.com/pytest-dev/pytest-xdist)** (>=3.5.0)
* **[scikeras](https://github.com/adriangb/scikeras)** (>=0.13.0)
* **[tensorflow](https://www.tensorflow.org/learn)** (>=2.16.1)

**Documentation**

* **[jupyter-contrib-nbextensions](https://github.com/ipython-contrib/jupyter_contrib_nbextensions)** (>=0.7.0)
* **[kaleido](https://github.com/plotly/Kaleido)** (>=0.2.1)
* **[mike](https://github.com/jimporter/mike)** (>=2.0.0)
* **[mkdocs](https://www.mkdocs.org/)** (>=1.5.3)
* **[mkdocs-autorefs](https://mkdocstrings.github.io/autorefs/)** (>=1.0.1)
* **[mkdocs-git-revision-date-localized-plugin](https://github.com/timvink/mkdocs-git-revision-date-localized-plugin)** (>=1.2.4)
* **[mkdocs-jupyter](https://github.com/danielfrg/mkdocs-jupyter)** (>=0.24.6)
* **[mkdocs-material](https://squidfunk.github.io/mkdocs-material/)** (>=9.5.13)
* **[mkdocs-material-extensions](https://pypi.org/project/mkdocs-material-extensions/)** (>=1.3.1)
* **[mkdocs-simple-hooks](https://github.com/aklajnert/mkdocs-simple-hooks)** (>=0.1.5)
* **[notebook](https://pypi.org/project/notebook/) (==6.4.12)**
* **[pymdown-extensions](https://github.com/facelessuser/pymdown-extensions)** (>=10.7.1)
* **[pyyaml](https://pyyaml.org/)** (>=6.0.1)
