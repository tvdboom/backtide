"""Backtide.

Author: Mavs
Description: Streamlit UI tests for all pages.

"""

import pytest

from streamlit.testing.v1 import AppTest


@pytest.fixture()
def _app(tmp_path):
    """Provide a working directory so the app finds its assets."""
    # The pages live under backtide/ui/ and reference images/ relative to CWD,
    # so we need to run from the repository root.
    import os

    original = os.getcwd()
    # Walk up until we find the images/ directory (repo root)
    root = original
    while not os.path.isdir(os.path.join(root, "images")):
        parent = os.path.dirname(root)
        if parent == root:
            break
        root = parent
    os.chdir(root)
    yield
    os.chdir(original)


class TestResultsPage:
    """Tests for the Results page."""

    def test_results_page_runs(self, _app):
        """The results page renders without raising an exception."""
        at = AppTest.from_file("backtide/ui/results.py", default_timeout=30)
        at.run()
        assert not at.exception, [e.value for e in at.exception]

    def test_results_page_has_title(self, _app):
        """The results page renders a title."""
        at = AppTest.from_file("backtide/ui/results.py", default_timeout=30)
        at.run()
        assert len(at.title) > 0


class TestStoragePage:
    """Tests for the Storage page."""

    def test_storage_page_runs(self, _app):
        """The storage page renders without raising an exception."""
        at = AppTest.from_file("backtide/ui/storage.py", default_timeout=30)
        at.run()
        assert not at.exception, [e.value for e in at.exception]

    def test_storage_page_has_title(self, _app):
        """The storage page renders a title."""
        at = AppTest.from_file("backtide/ui/storage.py", default_timeout=30)
        at.run()
        assert len(at.title) > 0


class TestExperimentPage:
    """Tests for the Experiment page."""

    def test_experiment_page_runs(self, _app):
        """The experiment page renders without raising an exception."""
        at = AppTest.from_file("backtide/ui/experiment.py", default_timeout=30)
        at.run()
        assert not at.exception, [e.value for e in at.exception]

    def test_experiment_page_has_title(self, _app):
        """The experiment page renders a title."""
        at = AppTest.from_file("backtide/ui/experiment.py", default_timeout=30)
        at.run()
        assert len(at.title) > 0


class TestDownloadPage:
    """Tests for the Download page."""

    def test_download_page_runs(self, _app):
        """The download page renders without raising an exception."""
        at = AppTest.from_file("backtide/ui/download.py", default_timeout=30)
        at.run()
        assert not at.exception, [e.value for e in at.exception]

    def test_download_page_has_title(self, _app):
        """The download page renders a title."""
        at = AppTest.from_file("backtide/ui/download.py", default_timeout=30)
        at.run()
        assert len(at.title) > 0
