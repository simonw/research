# Redis JavaScript Module Makefile
#
# This builds a Redis module that provides JavaScript scripting via mquickjs.
#
# Prerequisites:
#   - Redis source in REDIS_SRC (default: /tmp/redis)
#   - mquickjs source in MQUICKJS_SRC (default: /tmp/mquickjs)
#
# Usage:
#   make              - Build the module
#   make test         - Run tests
#   make clean        - Clean build artifacts
#   make install      - Install module (set INSTALL_DIR)

REDIS_SRC ?= /tmp/redis
MQUICKJS_SRC ?= /tmp/mquickjs
INSTALL_DIR ?= /usr/local/lib/redis/modules

CC = gcc
CFLAGS = -Wall -Wextra -g -O2 -fPIC -std=c99
CFLAGS += -I$(REDIS_SRC)/src -I$(MQUICKJS_SRC) -Isrc
CFLAGS += -D_GNU_SOURCE -fno-math-errno -fno-trapping-math

LDFLAGS = -shared -lm

# Module output
MODULE = redis-js.so

# Source files
SRC_DIR = src
BUILD_DIR = build

# mquickjs objects we need
MQUICKJS_OBJS = mquickjs.o dtoa.o libm.o cutils.o

# Our module source
MODULE_SRC = $(SRC_DIR)/redis_js.c

# Generated stdlib header
STDLIB_H = $(BUILD_DIR)/redis_js_stdlib.h

# Stdlib builder
STDLIB_BUILDER = $(BUILD_DIR)/redis_js_stdlib_builder

.PHONY: all clean test install

all: $(BUILD_DIR) $(MODULE)

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

# Build the stdlib generator first
$(STDLIB_BUILDER): $(SRC_DIR)/redis_js_stdlib.c $(BUILD_DIR)
	$(CC) -Wall -g -O2 -I$(MQUICKJS_SRC) -D_GNU_SOURCE \
		-o $(BUILD_DIR)/redis_js_stdlib.host.o -c $(SRC_DIR)/redis_js_stdlib.c
	$(CC) -Wall -g -O2 -I$(MQUICKJS_SRC) -D_GNU_SOURCE \
		-o $(BUILD_DIR)/mquickjs_build.host.o -c $(MQUICKJS_SRC)/mquickjs_build.c
	$(CC) -g -o $@ $(BUILD_DIR)/redis_js_stdlib.host.o $(BUILD_DIR)/mquickjs_build.host.o

# Generate the stdlib header
$(STDLIB_H): $(STDLIB_BUILDER)
	$(STDLIB_BUILDER) > $(STDLIB_H)

# Build mquickjs objects
$(BUILD_DIR)/mquickjs.o: $(MQUICKJS_SRC)/mquickjs.c $(BUILD_DIR)
	# First generate atoms header if needed
	@if [ ! -f $(MQUICKJS_SRC)/mquickjs_atom.h ]; then \
		cd $(MQUICKJS_SRC) && make mquickjs_atom.h; \
	fi
	$(CC) $(CFLAGS) -c -o $@ $<

$(BUILD_DIR)/dtoa.o: $(MQUICKJS_SRC)/dtoa.c $(BUILD_DIR)
	$(CC) $(CFLAGS) -c -o $@ $<

$(BUILD_DIR)/libm.o: $(MQUICKJS_SRC)/libm.c $(BUILD_DIR)
	$(CC) $(CFLAGS) -c -o $@ $<

$(BUILD_DIR)/cutils.o: $(MQUICKJS_SRC)/cutils.c $(BUILD_DIR)
	$(CC) $(CFLAGS) -c -o $@ $<

# Build main module
$(BUILD_DIR)/redis_js.o: $(MODULE_SRC) $(STDLIB_H) $(BUILD_DIR)
	$(CC) $(CFLAGS) -I$(BUILD_DIR) -c -o $@ $<

# Link the module
$(MODULE): $(BUILD_DIR)/redis_js.o $(addprefix $(BUILD_DIR)/,$(MQUICKJS_OBJS))
	$(CC) $(LDFLAGS) -o $@ $^

# Run tests
test: $(MODULE)
	@echo "Running tests..."
	./tests/run_tests.sh

# Clean build artifacts
clean:
	rm -rf $(BUILD_DIR) $(MODULE)

# Install the module
install: $(MODULE)
	mkdir -p $(INSTALL_DIR)
	cp $(MODULE) $(INSTALL_DIR)/
	@echo "Installed $(MODULE) to $(INSTALL_DIR)"
