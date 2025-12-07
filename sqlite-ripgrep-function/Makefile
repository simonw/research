# Makefile for SQLite ripgrep extension
#
# Build the C extension:
#   make
#
# Build with custom base directory:
#   make RIPGREP_BASE_DIR=/path/to/search
#
# Clean:
#   make clean

CC = gcc
CFLAGS = -Wall -Wextra -fPIC -O2
LDFLAGS = -shared

# Default base directory (can be overridden)
RIPGREP_BASE_DIR ?= /tmp
RIPGREP_TIME_LIMIT ?= 1.0

# SQLite header location (adjust if needed)
SQLITE_INCLUDE ?= /usr/include

# Add defines
CFLAGS += -DSQLITE_RIPGREP_BASE_DIR='"$(RIPGREP_BASE_DIR)"'
CFLAGS += -DSQLITE_RIPGREP_DEFAULT_TIME_LIMIT=$(RIPGREP_TIME_LIMIT)
CFLAGS += -I$(SQLITE_INCLUDE)

# Target
TARGET = sqlite_ripgrep.so

# Source files
SRCS = sqlite_ripgrep_ext.c

.PHONY: all clean test

all: $(TARGET)

$(TARGET): $(SRCS)
	$(CC) $(CFLAGS) $(LDFLAGS) -o $@ $<

clean:
	rm -f $(TARGET)

# Download sqlite3.h if not present
sqlite3ext.h:
	curl -sO https://raw.githubusercontent.com/sqlite/sqlite/master/src/sqlite3ext.h

# Run tests
test: $(TARGET)
	python3 test_sqlite_ripgrep.py

# Install to system
install: $(TARGET)
	cp $(TARGET) /usr/local/lib/

.PHONY: help
help:
	@echo "SQLite Ripgrep Extension Makefile"
	@echo ""
	@echo "Targets:"
	@echo "  all      - Build the extension (default)"
	@echo "  clean    - Remove built files"
	@echo "  test     - Run tests"
	@echo "  install  - Install to /usr/local/lib"
	@echo ""
	@echo "Variables:"
	@echo "  RIPGREP_BASE_DIR    - Base directory for searches (default: /tmp)"
	@echo "  RIPGREP_TIME_LIMIT  - Default time limit in seconds (default: 1.0)"
	@echo "  SQLITE_INCLUDE      - Path to SQLite headers (default: /usr/include)"
	@echo ""
	@echo "Example:"
	@echo "  make RIPGREP_BASE_DIR=/home/user/code RIPGREP_TIME_LIMIT=2.0"
