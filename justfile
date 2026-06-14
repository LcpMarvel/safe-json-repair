# safe-json-repair — task runner (the cross-ecosystem entry point).
#
#   just            list all recipes
#   just test       run Rust + JS + Python tests
#   just release-dry / just release CODE   full publish, with npm 2FA code
#
# This orchestrates ALL ecosystems (cargo + the npm/wasm binding + the
# PyO3/maturin binding), which a cargo alias can't do. `-j 2` matches this
# repo's build-parallelism convention.

js := justfile_directory() / "bindings/js"
py := justfile_directory() / "bindings/python"

# List available recipes.
default:
    @just --list

# ---------------------------------------------------------------- build -------

# Build the Rust core (debug).
build-core:
    cargo build -j 2

# Build the npm package (rebuilds the speed-tuned wasm, then bundles).
build-js:
    cd {{js}} && npm run build

# Build the Python wheel and install it into bindings/python/.venv (created if
# missing). Uses a wheel install rather than `maturin develop` so it works
# reliably in a uv-created venv.
build-py:
    cd {{py}} && test -d .venv || uv venv .venv
    cd {{py}} && uv pip install --python .venv maturin pytest pip
    cd {{py}} && CARGO_BUILD_JOBS=2 .venv/bin/maturin build --out dist
    cd {{py}} && .venv/bin/pip install --force-reinstall dist/*.whl

# Build everything.
build: build-core build-js build-py

# ----------------------------------------------------------------- test -------

# Run the Rust test suite (corpus + differential + robustness + doctests).
test-rust:
    cargo test -j 2

# Run the JS binding tests (rebuilds wasm first, then runs the shared corpus).
test-js:
    cd {{js}} && npm test

# Run the Python binding tests (rebuilds + installs the wheel, then runs the
# shared corpus + API tests).
test-py: build-py
    cd {{py}} && .venv/bin/pytest -q

# Run everything: Rust + JS + Python.
test: test-rust test-js test-py

# ----------------------------------------------------------------- bench ------

# Quick criterion run (short sampling).
bench:
    cargo bench -j 2 --bench repair -- --measurement-time 3 --sample-size 30

# Full criterion run.
bench-full:
    cargo bench -j 2

# --------------------------------------------------------------- publish ------

# Dry-run the crate release (package + verify build, no upload).
publish-crate-dry:
    cargo publish -p safe-json-repair --dry-run --allow-dirty -j 2

# Publish the core crate to crates.io (requires a prior `cargo login`).
publish-crate:
    cargo publish -p safe-json-repair --allow-dirty -j 2

# Dry-run the npm release (shows tarball contents, no upload).
publish-npm-dry:
    cd {{js}} && npm publish --dry-run

# Publish the npm package. npm prompts for your 2FA code interactively
# (run it in a real terminal so the OTP prompt can appear).
publish-npm:
    cd {{js}} && npm publish

# Dry-run the PyPI release (builds the wheel + sdist, no upload).
publish-pypi-dry:
    cd {{py}} && CARGO_BUILD_JOBS=2 .venv/bin/maturin build --release --out dist --sdist

# Publish the Python package to PyPI (uses a configured token / ~/.pypirc).
# NOTE: a real release builds per-platform wheels in CI (cibuildwheel/maturin
# action); this single-platform recipe is for local/manual pushes.
publish-pypi:
    cd {{py}} && CARGO_BUILD_JOBS=2 .venv/bin/maturin publish
