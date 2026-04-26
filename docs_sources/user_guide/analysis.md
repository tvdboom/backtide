# Plots
-------

Backtide provides many plotting methods to analyze the data or compare the
model performances. Descriptions and examples can be found in the API
section. ATOM mainly uses the [plotly](https://plotly.com/python/) library for plotting. Plotly makes
interactive, publication-quality graphs that are rendered using HTML.

<br>

## Parameters

Apart from the plot-specific parameters, all plots have five parameters in common:

* The `title` parameter adds a title to the plot. The default value doesn't
  show any title. Provide a configuration (as dictionary) to customize its
  appearance, e.g., `#!python title=dict(text="Awesome plot", color="red")`.
  Read more in plotly's [documentation](https://plotly.com/python/figure-labels/).
* The `legend` parameter is used to show/hide, position or customize the
  plot's legend. Provide a configuration (as dictionary) to customize its
  appearance (e.g., `#!python legend=dict(title="Title for legend", title_font_color="red")`)
  or choose one of the following locations:

    - upper left
    - upper right
    - lower left
    - lower right
    - upper center
    - lower center
    - center left
    - center right
    - center
    - out: Position the legend outside the axis, on the right hand side. This
      is plotly's default position. Note that this shrinks the size of the axis
      to fit both legend and axes in the specified `figsize`.

* The `figsize` parameter adjust the plot's size.
* The `filename` parameter is used to save the plot.
* The `display` parameter determines whether to show or return the plot.

<br>

## Aesthetics

The plot's aesthetics are controlled through the `plots` section of the
[configuration][configuration]. The default values are:

* **template:** `"plotly"` — Plotly template for figure styling.
* **palette:** Blue-to-teal gradient. Colors cycle when there are more
  traces than entries. `["rgb(13, 71, 161)", "rgb(2, 136, 209)", "rgb(0,
  172, 193)", "rgb(0, 137, 123)", "rgb(56, 142, 60)", "rgb(129, 199, 132)"]`
* **title_fontsize:** `22` — Font size (px) for plot titles.
* **label_fontsize:** `20` — Font size (px) for axis labels and legends.
* **tick_fontsize:** `14` — Font size (px) for axis tick labels.

To change these values, set them in your configuration file or programmatically:

```python
from backtide.config import get_config, set_config

cfg = get_config()
cfg.plots.template = "plotly_dark"
cfg.plots.title_fontsize = 28
set_config(cfg)
```
