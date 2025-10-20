#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== minijinja vs jinja2 Benchmark Suite ===${NC}"
echo ""

# Configuration
ITERATIONS=200
NUM_PRODUCTS=50
RESULTS_DIR="./results"
CHARTS_DIR="./charts"
MINIJINJA_REPO="https://github.com/mitsuhiko/minijinja.git"
MINIJINJA_DIR="./minijinja-repo"

# Create directories
mkdir -p "$RESULTS_DIR"
mkdir -p "$CHARTS_DIR"

# Check if uv is installed
if ! command -v uv &> /dev/null; then
    echo -e "${RED}Error: uv is not installed${NC}"
    echo "Please install uv: https://github.com/astral-sh/uv"
    exit 1
fi

echo -e "${YELLOW}Step 1: Downloading minijinja repository${NC}"
if [ -d "$MINIJINJA_DIR" ]; then
    echo "Repository already exists, skipping download..."
else
    echo "Downloading minijinja source from GitHub..."
    curl -L https://github.com/mitsuhiko/minijinja/archive/refs/heads/main.tar.gz -o minijinja.tar.gz
    tar -xzf minijinja.tar.gz
    mv minijinja-main "$MINIJINJA_DIR"
    rm minijinja.tar.gz
    echo "Download complete!"
fi

echo ""
echo -e "${YELLOW}Step 2: Setting up Python 3.14 environment${NC}"

# Create Python 3.14 environment
if [ ! -d ".venv-3.14" ]; then
    uv venv .venv-3.14 --python 3.14
fi

echo "Installing dependencies for Python 3.14..."
source .venv-3.14/bin/activate

# Install jinja2
uv pip install jinja2

# Build and install minijinja from source
echo "Building minijinja from source..."
cd "$MINIJINJA_DIR/minijinja-py"
uv pip install maturin
maturin develop --release
cd ../..

# Install visualization dependencies
uv pip install matplotlib numpy

deactivate

echo ""
echo -e "${YELLOW}Step 3: Setting up Python 3.14t (free-threaded) environment${NC}"

# Create Python 3.14t environment
if [ ! -d ".venv-3.14t" ]; then
    uv venv .venv-3.14t --python 3.14t
fi

echo "Installing dependencies for Python 3.14t..."
source .venv-3.14t/bin/activate

# Install jinja2
uv pip install jinja2

# Build and install minijinja from source
echo "Building minijinja from source for free-threaded Python..."
cd "$MINIJINJA_DIR/minijinja-py"
uv pip install maturin
maturin develop --release
cd ../..

# Install visualization dependencies
uv pip install matplotlib numpy

deactivate

echo ""
echo -e "${YELLOW}Step 4: Running benchmarks${NC}"

# Benchmark 1: jinja2 on Python 3.14
echo -e "${GREEN}Running jinja2 on Python 3.14...${NC}"
source .venv-3.14/bin/activate
python benchmark.py \
    --engine jinja2 \
    --iterations $ITERATIONS \
    --num-products $NUM_PRODUCTS \
    --output "$RESULTS_DIR/jinja2-3.14.json"
deactivate

# Benchmark 2: minijinja on Python 3.14
echo -e "${GREEN}Running minijinja on Python 3.14...${NC}"
source .venv-3.14/bin/activate
python benchmark.py \
    --engine minijinja \
    --iterations $ITERATIONS \
    --num-products $NUM_PRODUCTS \
    --output "$RESULTS_DIR/minijinja-3.14.json"
deactivate

# Benchmark 3: jinja2 on Python 3.14t
echo -e "${GREEN}Running jinja2 on Python 3.14t (free-threaded)...${NC}"
source .venv-3.14t/bin/activate
python benchmark.py \
    --engine jinja2 \
    --iterations $ITERATIONS \
    --num-products $NUM_PRODUCTS \
    --output "$RESULTS_DIR/jinja2-3.14t.json"
deactivate

# Benchmark 4: minijinja on Python 3.14t
echo -e "${GREEN}Running minijinja on Python 3.14t (free-threaded)...${NC}"
source .venv-3.14t/bin/activate
python benchmark.py \
    --engine minijinja \
    --iterations $ITERATIONS \
    --num-products $NUM_PRODUCTS \
    --output "$RESULTS_DIR/minijinja-3.14t.json"
deactivate

echo ""
echo -e "${YELLOW}Step 5: Generating visualizations${NC}"
source .venv-3.14/bin/activate
python visualize_results.py \
    --results-dir "$RESULTS_DIR" \
    --output-dir "$CHARTS_DIR"
deactivate

echo ""
echo -e "${GREEN}=== Benchmark Complete ===${NC}"
echo ""
echo "Results saved to: $RESULTS_DIR"
echo "Charts saved to: $CHARTS_DIR"
echo ""
echo -e "${YELLOW}Summary:${NC}"

# Display summary
for result_file in "$RESULTS_DIR"/*.json; do
    if [ -f "$result_file" ]; then
        engine=$(jq -r '.engine' "$result_file")
        mean=$(jq -r '.mean_ms' "$result_file")
        ft=$(jq -r '.free_threaded' "$result_file")
        py_impl=$(jq -r '.python_implementation' "$result_file")

        if [ "$ft" == "true" ]; then
            py_version="3.14t"
        else
            py_version="3.14"
        fi

        printf "  %-20s (Python %s): %.3f ms\n" "$engine" "$py_version" "$mean"
    fi
done

echo ""
echo "View the charts in the '$CHARTS_DIR' directory!"
