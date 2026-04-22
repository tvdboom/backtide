# Getting started
-----------------

## Installation

Install backtide's newest release easily via `pip`:

    pip install -U backtide

or via `conda`:

    conda install -c conda-forge backtide

<br style="display: block; margin-top: 2em; content: ' '">

**Latest source**

Sometimes, new features and bug fixes are already implemented in the
`development` branch, but waiting for the next release to be made
available. If you can't wait for that, it's possible to install the
package directly from git.

    pip install git+https://github.com/tvdboom/backtide.git@development#egg=backtide

Don't forget to include `#egg=backtide` to explicitly name the project,
this way pip can track metadata for it without having to have run the
`setup.py` script.

<br style="display: block; margin-top: 2em; content: ' '">

**Optional dependencies**

Some specific functionalities or configuration options require the installation
of additional libraries. Install all [optional dependencies][optional] with:

    pip install -U backtide[full]

<br style="display: block; margin-top: 2em; content: ' '">

**Contributing**

If you are planning to [contribute][contributing] to the project, you'll need the
[development dependencies][development]. Install them with:

    pip install -U backtide[dev]

Click [here](https://pypi.org/simple/backtide/) for a complete list of package files for all versions published
on PyPI.

<br><br>


## Usage

There are three ways to use backtide:

<br>

**Via the application**

Backtide ships with an interactive [Streamlit](https://streamlit.io/) application. Launch it from
the terminal with:

    backtide launch

The app provides a graphical interface for configuring experiments, visualizing
results and managing stored data. Read more in the [user guide][application].

<br>

**Via the CLI**

The `backtide` command-line interface lets you download data, run backtests and
manage storage directly from the terminal.

    backtide download AAPL MSFT --interval 1d
    backtide run experiment.toml

Run `backtide --help` to see all available commands.

<br>

**Via Python**

Import backtide in any Python script or notebook for full programmatic control.

```pycon
from backtide.data import resolve_profiles, download_bars
from backtide.storage import query_bars

profiles = resolve_profiles(["AAPL", "MSFT"], "stocks", "1d")
result = download_bars(profiles)

data = query_bars("AAPL")
print(data.head())
```
