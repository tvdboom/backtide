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

The plot's aesthetics can be customized using the plot attributes prior
to calling the plotting method, e.g., `#!python atom.title_fontsize = 30`.
The default values are:

* **palette:** ["rgb(0, 98, 98)", "rgb(56, 166, 165)", "rgb(115, 175, 72)",
  "rgb(237, 173, 8)", "rgb(225, 124, 5)", "rgb(204, 80, 62)", "rgb(148, 52, 110)",
  "rgb(111, 64, 112)", "rgb(102, 102, 102)"]
* **title_fontsize:** 24
* **label_fontsize:** 16
* **tick_fontsize:** 12
