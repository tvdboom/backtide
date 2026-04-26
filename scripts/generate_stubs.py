"""Generate .pyi stub files for the backtide.core Rust extension module.

This script introspects the compiled PyO3 module (`backtide.core`) and
generates Python type-stub files (.pyi) for each submodule. It leverages
PyO3's `experimental-inspect` feature to extract __text_signature__ and
docstrings.

Usage
-----
    # Build the extension first (if not already built)
    maturin develop

    # Generate stubs
    python scripts/generate_stubs.py

    # Check that stubs are up-to-date (for CI / pre-commit)
    python scripts/generate_stubs.py --check

The stubs are written to:
    backtide/core/__init__.pyi
    backtide/core/backtest.pyi
    backtide/core/config.pyi
    backtide/core/data.pyi
    backtide/core/storage.pyi
    backtide/core/utils.pyi

"""

from __future__ import annotations

import argparse
import difflib
import importlib
import inspect
from pathlib import Path
import re
import sys
from types import ModuleType

# ─────────────────────────────────────────────────────────────────────────────
# Constants
# ─────────────────────────────────────────────────────────────────────────────

ROOT = Path(__file__).resolve().parent.parent
STUB_DIR = ROOT / "src" / "backtide" / "core"

# Submodules to generate stubs for.
SUBMODULES = ["analysis", "backtest", "config", "data", "storage", "utils"]

# PyO3 built-in dunder methods we always skip (they have no useful stub).
SKIP_MEMBERS = {
    "__module__",
    "__qualname__",
    "__dict__",
    "__weakref__",
    "__subclasshook__",
    "__init_subclass__",
    "__class__",
    "__delattr__",
    "__dir__",
    "__format__",
    "__getattribute__",
    "__reduce__",
    "__reduce_ex__",
    "__setattr__",
    "__sizeof__",
    "__class_getitem__",
}

# ─────────────────────────────────────────────────────────────────────────────
# Docstring type extraction
# ─────────────────────────────────────────────────────────────────────────────

# Regex for a NumPy-style attribute line: "name : type"
_ATTR_RE = re.compile(r"^(\w+)\s*:\s*(.+)$")

# Strip doc-reference brackets like [ClassName] → ClassName, but NOT
# generic-type brackets like list[str] (preceded by a word character).
_REF_RE = re.compile(r"(?<!\w)\[(\w+)]")


def _clean_type(raw: str) -> str:
    """Convert a docstring type annotation to a valid Python stub type.

    Handles patterns like:
      - `str | [Currency]` → `str | Currency`
      - `list[str | [Instrument]]` → `list[str | Instrument]`
      - `dict[str | [InstrumentType], str | [Provider]] | None` → ...
      - `int or None` → `int | None`
      - `str, default="USD"` → `str`
    """
    # Strip default values:  "str, default=..."  → "str"
    t = re.sub(r",\s*default\s*=.*$", "", raw).strip()

    # Replace "or" with "|"
    t = re.sub(r"\bor\b", "|", t)

    # Remove doc-reference brackets:  [Foo]  → Foo
    t = _REF_RE.sub(r"\1", t)

    # Clean up whitespace around |
    t = re.sub(r"\s*\|\s*", " | ", t)

    return t.strip()


def _parse_attributes_from_doc(doc: str | None) -> dict[str, str]:
    """Parse attribute types from a NumPy-style Attributes section."""
    if not doc:
        return {}

    doc = _clean_rust_docs(doc)

    attrs: dict[str, str] = {}
    lines = doc.split("\n")
    in_attrs = False

    for line in lines:
        stripped = line.strip()

        if stripped == "Attributes":
            in_attrs = True
            continue

        if in_attrs and stripped.startswith("---"):
            continue

        # End of Attributes section on new section header
        if in_attrs and stripped in (
            "Parameters",
            "Returns",
            "Raises",
            "See Also",
            "Examples",
            "Notes",
            "References",
        ):
            break

        if in_attrs:
            m = _ATTR_RE.match(stripped)
            if m:
                name, raw_type = m.group(1), m.group(2)
                attrs[name] = _clean_type(raw_type)

    return attrs


def _extract_return_type_from_doc(doc: str | None, class_name: str = "") -> str | None:
    """Extract the return type from a NumPy-style docstring's Returns section."""
    if not doc:
        return None

    doc = _clean_rust_docs(doc)

    lines = doc.split("\n")
    in_returns = False
    for line in lines:
        stripped = line.strip()
        if stripped == "Returns":
            in_returns = True
            continue
        if in_returns and stripped.startswith("---"):
            continue
        if in_returns and stripped:
            ret_type = _clean_type(stripped)
            if class_name:
                ret_type = re.sub(r"\bself\b", class_name, ret_type)
            return ret_type
        if in_returns and not stripped:
            in_returns = False

    return None


# ─────────────────────────────────────────────────────────────────────────────
# Signature helpers
# ─────────────────────────────────────────────────────────────────────────────


def _clean_signature(sig: str, name: str) -> str:
    """Clean up a __text_signature__ string from PyO3."""
    sig = sig.replace("$self", "self").replace("$cls", "cls").replace("$type", "cls")

    if sig.startswith(f"{name}("):
        sig = sig[len(name) :]

    return sig


def _parse_text_signature(text_sig: str | None, name: str) -> str | None:
    """Parse a __text_signature__ into a clean signature string."""
    if text_sig is None:
        return None
    return _clean_signature(text_sig.strip(), name)


# ─────────────────────────────────────────────────────────────────────────────
# Docstring formatting
# ─────────────────────────────────────────────────────────────────────────────

# Rust triple-slash doc comment prefix that sometimes leaks through PyO3.
_RUST_DOC_RE = re.compile(r"^(\s*)///\s?", re.MULTILINE)

# Mojibake arrow: PyO3 on Windows can turn the Rust `->` into U+2192 (→)
# whose UTF-8 bytes (\xe2\x86\x92) are then misinterpreted as CP-1252,
# yielding the three-character sequence â (U+00E2) † (U+2020) ' (U+2019).
_MOJIBAKE_ARROW = "\u00e2\u2020\u2019"  # â†'
_UNICODE_ARROW = "\u2192"  # →


def _clean_rust_docs(text: str) -> str:
    """Clean `/// ` prefixes and misinterpreted bytes that leak into docstrings."""
    text = _RUST_DOC_RE.sub(r"\1", text)
    text = text.replace(_MOJIBAKE_ARROW, "->")
    text = text.replace(_UNICODE_ARROW, "->")
    return text


def _format_docstring(doc: str | None, indent: str = "    ") -> str:
    """Format a docstring for inclusion in a stub file."""
    if not doc:
        return f"{indent}...\n"

    doc = _clean_rust_docs(doc)

    lines = doc.strip().split("\n")
    if len(lines) == 1:
        return f'{indent}"""{lines[0]}"""\n'

    result = f'{indent}"""{lines[0]}\n'
    for line in lines[1:]:
        if line.strip():
            result += f"{indent}{line}\n"
        else:
            result += "\n"
    result += f'\n{indent}"""\n'
    return result


# ─────────────────────────────────────────────────────────────────────────────
# Introspection helpers
# ─────────────────────────────────────────────────────────────────────────────


def _is_pyclass(obj: object) -> bool:
    """Check if an object is a PyO3 class (not a function or module)."""
    return isinstance(obj, type) and not isinstance(obj, ModuleType)


def _is_pyfunction(obj: object) -> bool:
    """Check if an object is a PyO3-wrapped function."""
    return callable(obj) and not isinstance(obj, type)


def _get_descriptor_names(cls: type) -> set[str]:
    """Return the set of attribute names exposed via getset descriptors."""
    attrs = set()
    for name in dir(cls):
        if name.startswith("_"):
            continue
        try:
            member = getattr(cls, name)
        except AttributeError:
            continue
        type_name = type(member).__name__
        if type_name in ("getset_descriptor", "member_descriptor") or isinstance(member, property):
            attrs.add(name)
    return attrs


def _get_enum_variants(cls: type) -> list[str]:
    """Return sorted names of class-level attributes that are instances of *cls*.

    PyO3 enum variants (`#[pyclass]` with `#[pyo3(enum)]`) are exposed as
    class attributes whose value is an instance of the enum class itself.

    """
    variants: list[str] = []
    for name in dir(cls):
        if name.startswith("_"):
            continue
        try:
            member = getattr(cls, name)
        except AttributeError:
            continue
        if isinstance(member, cls):
            variants.append(name)
    return sorted(variants)


# ─────────────────────────────────────────────────────────────────────────────
# Stub generators
# ─────────────────────────────────────────────────────────────────────────────


def _generate_method_stub(
    name: str,
    obj: object,
    class_name: str = "",
    indent: str = "    ",
) -> str:
    """Generate a stub for a single method or function."""
    text_sig = getattr(obj, "__text_signature__", None)
    doc = getattr(obj, "__doc__", None)

    sig = _parse_text_signature(text_sig, name)
    ret_type = _extract_return_type_from_doc(doc, class_name=class_name)

    if sig:
        ret_str = f" -> {ret_type}" if ret_type else ""
        result = f"{indent}def {name}{sig}{ret_str}:\n"
        if doc and not name.startswith("__"):
            result += _format_docstring(doc, indent=indent + "    ")
        else:
            result += f"{indent}    ...\n"
        return result

    # Fallback: generic signature
    if name == "__new__":
        return f"{indent}def __new__(cls, *args, **kwargs): ...\n"
    if name.startswith("__") and name.endswith("__"):
        return f"{indent}def {name}(self, *args, **kwargs): ...\n"

    ret_str = f" -> {ret_type}" if ret_type else ""
    result = f"{indent}def {name}(self, *args, **kwargs){ret_str}:\n"
    if doc and not name.startswith("__"):
        result += _format_docstring(doc, indent=indent + "    ")
    else:
        result += f"{indent}    ...\n"
    return result


def _generate_class_stub(name: str, cls: type, all_doc_types: dict[str, str]) -> str:
    """Generate a stub for an entire PyO3 class."""
    lines: list[str] = [f"class {name}:\n"]

    doc = getattr(cls, "__doc__", None)
    if doc:
        lines.append(_format_docstring(doc))
        lines.append("\n")  # Blank line between docstring and class body

    # ── Attributes ──────────────────────────────────────────────────────

    descriptor_names = _get_descriptor_names(cls)
    doc_types = _parse_attributes_from_doc(doc)
    enum_variants = _get_enum_variants(cls)

    for attr_name in sorted(descriptor_names):
        # 1st: own docstring, 2nd: cross-class lookup, 3rd: Any
        attr_type = doc_types.get(attr_name) or all_doc_types.get(attr_name, "Any")
        lines.append(f"    {attr_name}: {attr_type}\n")

    if descriptor_names:
        lines.append("\n")

    # ── Enum variants ───────────────────────────────────────────────────

    if enum_variants:
        lines.extend(f"    {variant}: ClassVar[{name}]\n" for variant in enum_variants)
        lines.append("\n")

    # ── Methods ─────────────────────────────────────────────────────────

    methods_added: set[str] = set()

    for member_name in sorted(dir(cls)):
        if member_name in SKIP_MEMBERS:
            continue
        if member_name in descriptor_names:
            continue
        if member_name in enum_variants:
            continue

        try:
            member = getattr(cls, member_name)
        except AttributeError:
            continue

        if not callable(member):
            continue

        type_name = type(member).__name__
        if type_name in ("getset_descriptor", "member_descriptor"):
            continue

        raw = inspect.getattr_static(cls, member_name, None)
        is_classmethod = isinstance(raw, classmethod)
        is_staticmethod = isinstance(raw, staticmethod)

        if is_classmethod:
            lines.append("    @classmethod\n")
        elif is_staticmethod:
            lines.append("    @staticmethod\n")

        stub = _generate_method_stub(member_name, member, class_name=name, indent="    ")
        lines.append(stub)
        methods_added.add(member_name)

    if not descriptor_names and not methods_added:
        lines.append("    ...\n")

    lines.append("\n")
    return "".join(lines)


def _wrap_signature(name: str, sig: str, ret_str: str, indent: str = "") -> str:
    """Wrap a long function signature into one-parameter-per-line style."""
    # sig looks like "(param1, param2, *, kw=default)"
    inner = sig[1:-1]  # strip outer parens
    params = [p.strip() for p in _split_params(inner)]
    lines = [f"{indent}def {name}(\n"]
    lines.extend(f"{indent}    {param},\n" for param in params)
    lines.append(f"{indent}){ret_str}:\n")
    return "".join(lines)


def _split_params(params_str: str) -> list[str]:
    """Split a parameter string on commas, respecting brackets."""
    parts: list[str] = []
    depth = 0
    current: list[str] = []
    for ch in params_str:
        if ch in ("(", "[", "{"):
            depth += 1
        elif ch in (")", "]", "}"):
            depth -= 1
        if ch == "," and depth == 0:
            parts.append("".join(current))
            current = []
        else:
            current.append(ch)
    if current:
        parts.append("".join(current))
    return parts


def _generate_function_stub(name: str, func: object) -> str:
    """Generate a stub for a module-level function."""
    text_sig = getattr(func, "__text_signature__", None)
    doc = getattr(func, "__doc__", None)

    sig = _parse_text_signature(text_sig, name)
    ret_type = _extract_return_type_from_doc(doc)

    ret_str = f" -> {ret_type}" if ret_type else ""

    if sig:
        def_line = f"def {name}{sig}{ret_str}:\n"
        if len(def_line.rstrip()) > 99:
            # Wrap parameters one-per-line like ruff-format does.
            def_line = _wrap_signature(name, sig, ret_str)
        result = def_line
    else:
        result = f"def {name}(*args, **kwargs){ret_str}:\n"

    if doc:
        result += _format_docstring(doc)
    else:
        result += "    ...\n"

    return result + "\n"


# ─────────────────────────────────────────────────────────────────────────────
# Main generation logic
# ─────────────────────────────────────────────────────────────────────────────


def generate_submodule_stub(submodule_name: str) -> str:
    """Generate the full .pyi content for a given submodule."""
    module_path = f"backtide.core.{submodule_name}"

    try:
        mod = importlib.import_module(module_path)
    except ImportError as e:
        print(f"  SKIP Cannot import {module_path}: {e}", file=sys.stderr)
        return ""

    lines: list[str] = [
        f'"""Type stubs for `{module_path}` (auto-generated)."""\n',
        "\n",
    ]

    classes: list[tuple[str, type]] = []
    functions: list[tuple[str, object]] = []

    for attr_name in sorted(dir(mod)):
        if attr_name.startswith("_"):
            continue
        obj = getattr(mod, attr_name)
        if _is_pyclass(obj):
            classes.append((attr_name, obj))
        elif _is_pyfunction(obj):
            functions.append((attr_name, obj))

    # Build a merged index of attribute types across all classes in this
    # module. When a class exposes delegate properties (e.g. InstrumentProfile
    # forwarding Instrument attributes), their types can be resolved from the
    # class that actually documents them.
    all_doc_types: dict[str, str] = {}
    for _, cls in classes:
        doc = getattr(cls, "__doc__", None)
        for attr, typ in _parse_attributes_from_doc(doc).items():
            all_doc_types.setdefault(attr, typ)

    all_names = [n for n, _ in classes] + [n for n, _ in functions]
    if all_names:
        # Format __all__ to match ruff-format style: single line if short,
        # one entry per line if the list exceeds the line-length limit.
        single_line = "[" + ", ".join(f'"{n}"' for n in all_names) + "]"
        if len(f"__all__ = {single_line}") <= 99:
            lines.append(f"__all__ = {single_line}\n\n")
        else:
            items = "".join(f'    "{n}",\n' for n in all_names)
            lines.append(f"__all__ = [\n{items}]\n\n")

    body_lines: list[str] = []
    for cls_name, cls in classes:
        body_lines.append(_generate_class_stub(cls_name, cls, all_doc_types))

    for fn_name, fn in functions:
        body_lines.append(_generate_function_stub(fn_name, fn))

    body = "".join(body_lines)

    # Only emit typing imports that are actually used in the body.
    typing_imports = [name for name in ("Any", "ClassVar") if re.search(rf"\b{name}\b", body)]
    if typing_imports:
        lines.append(f"from typing import {', '.join(typing_imports)}\n")
        lines.append("\n")

    # ── Third-party imports ─────────────────────────────────────────────

    needs_numpy = "np." in body
    needs_pandas = "pd." in body
    needs_polars = "pl." in body

    if needs_numpy or needs_pandas or needs_polars:
        if needs_numpy:
            lines.append("import numpy as np\n")
        if needs_pandas:
            lines.append("import pandas as pd\n")
        if needs_polars:
            lines.append("import polars as pl\n")
        lines.append("\n")

    # ── Cross-module type imports ────────────────────────────────────────

    # Collect class names defined in *this* submodule so we can exclude them.
    local_names = {n for n, _ in classes}

    # Strip docstrings so cross-references in "See Also" sections don't
    # cause false-positive imports.
    body_no_docs = re.sub(r'""".*?"""', "", body, flags=re.DOTALL)

    cross_imports: dict[str, list[str]] = {}
    for other_sub in SUBMODULES:
        if other_sub == submodule_name:
            continue
        other_mod_path = f"backtide.core.{other_sub}"
        try:
            other_mod = importlib.import_module(other_mod_path)
        except ImportError:
            continue
        for attr_name in sorted(dir(other_mod)):
            if attr_name.startswith("_"):
                continue
            obj = getattr(other_mod, attr_name)
            if (
                _is_pyclass(obj)
                and attr_name not in local_names
                and re.search(rf"\b{attr_name}\b", body_no_docs)
            ):
                cross_imports.setdefault(other_sub, []).append(attr_name)

    for other_sub in sorted(cross_imports):
        names = ", ".join(sorted(cross_imports[other_sub]))
        lines.append(f"from backtide.core.{other_sub} import {names}\n")
    if cross_imports:
        lines.append("\n")

    lines.append(body)
    return "".join(lines).rstrip("\n") + "\n"


def generate_init_stub() -> str:
    """Generate the backtide/core/__init__.pyi stub."""
    lines = [
        '"""Type stubs for `backtide.core` (auto-generated)."""\n',
        "\n",
    ]
    lines.extend(f"from backtide.core import {sub} as {sub}\n" for sub in SUBMODULES)
    return "".join(lines)


def main():
    """Entry point: generate all stub files."""
    parser = argparse.ArgumentParser(description="Generate .pyi stubs for backtide.core")
    parser.add_argument(
        "--check",
        action="store_true",
        help="Verify that existing stubs match what would be generated. "
        "Exits with code 1 if any file is out of date (useful for CI / pre-commit).",
    )
    args = parser.parse_args()

    sys.path.insert(0, str(ROOT))
    STUB_DIR.mkdir(parents=True, exist_ok=True)

    # Collect all (path, content) pairs to write / compare.
    stubs: list[tuple[Path, str]] = [(STUB_DIR / "__init__.pyi", generate_init_stub())]
    for sub in SUBMODULES:
        content = generate_submodule_stub(sub)
        if content:
            stubs.append((STUB_DIR / f"{sub}.pyi", content))

    if args.check:
        # ── Check mode ──────────────────────────────────────────────────

        out_of_date: list[str] = []

        for path, expected in stubs:
            rel = path.relative_to(ROOT)
            if not path.exists():
                out_of_date.append(str(rel))
                print(f"  FAIL {rel}  (missing)")
                continue

            existing = path.read_text(encoding="utf-8")
            if existing != expected:
                out_of_date.append(str(rel))
                print(f"  FAIL {rel}  (out of date)")

                # Show a short unified diff so the developer can see what changed.
                diff = difflib.unified_diff(
                    existing.splitlines(keepends=True),
                    expected.splitlines(keepends=True),
                    fromfile=f"a/{rel}",
                    tofile=f"b/{rel}",
                    n=3,
                )
                sys.stdout.writelines(diff)
            else:
                print(f"  OK {rel}")

        if out_of_date:
            print(
                f"\n{len(out_of_date)} stub(s) out of date. "
                "Run `python scripts/generate_stubs.py` to regenerate.",
                file=sys.stderr,
            )
            raise SystemExit(1)
        else:
            print("\nAll stubs are up to date.")
    else:
        # ── Generate mode ───────────────────────────────────────────────

        print("Generating .pyi stubs for backtide.core...")

        for path, content in stubs:
            path.write_text(content, encoding="utf-8")
            print(f"  OK {path.relative_to(ROOT)}")

        print("\nDone! Stubs written to backtide/core/")


if __name__ == "__main__":
    main()
