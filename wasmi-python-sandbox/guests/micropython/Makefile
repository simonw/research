# Build MicroPython as a wasm32 reactor for the wasmi sandbox.
#
#   make MICROPYTHON_TOP=/path/to/micropython WASI_SDK=/path/to/wasi-sdk
#
# Step 1 uses MicroPython's "embed" port to generate a self-contained source
# tree (micropython_embed/). Step 2 compiles it with wasi-sdk's clang using
# LLVM's emscripten-style setjmp/longjmp lowering.

MICROPYTHON_TOP ?= ../../../../micropython
WASI_SDK ?= /opt/wasi-sdk
CC = $(WASI_SDK)/bin/clang
EMBED_DIR = micropython_embed
OUT = micropython.wasm

STACK_SIZE ?= 1048576

CFLAGS += --target=wasm32-wasip1 -O2 -DNDEBUG
CFLAGS += -mllvm -enable-emscripten-sjlj
CFLAGS += -Iinclude -I. -I$(EMBED_DIR) -I$(EMBED_DIR)/port
CFLAGS += -Wall -Wno-unused-function -std=gnu11 -fno-common

LDFLAGS += -mexec-model=reactor
LDFLAGS += -Wl,--export-table -Wl,--export=malloc -Wl,--export=free
LDFLAGS += -Wl,--allow-undefined
LDFLAGS += -Wl,-z,stack-size=$(STACK_SIZE) -Wl,--stack-first
LDFLAGS += -Wl,--export=__stack_low -Wl,--export=__stack_high -Wl,--export=__stack_pointer

SRC = main.c mphalport.c modhost.c sjlj_runtime.c
SRC += $(EMBED_DIR)/extmod/modjson.c
SRC += $(wildcard $(EMBED_DIR)/py/*.c)
# gchelper_generic.c is not needed: collections scan the shadow stack from main.c
# port/embed_util.c and port/mphalport.c are replaced by main.c / mphalport.c here
OBJ = $(SRC:.c=.o)

all: $(OUT)

$(EMBED_DIR)/genhdr/qstrdefs.generated.h: mpconfigport.h modhost.c
	$(MAKE) -f micropython_embed.mk MICROPYTHON_TOP=$(MICROPYTHON_TOP)
	cp $(MICROPYTHON_TOP)/extmod/modjson.c $(EMBED_DIR)/extmod/

embed: $(EMBED_DIR)/genhdr/qstrdefs.generated.h

$(OUT): embed
	$(CC) $(CFLAGS) $(LDFLAGS) -o $@ $(SRC)
	mkdir -p ../../wasmi_sandbox/python/wasmi_sandbox/guests && cp $@ ../../wasmi_sandbox/python/wasmi_sandbox/guests/
	ls -la $@

clean:
	rm -rf $(EMBED_DIR) build-embed $(OUT)

.PHONY: all embed clean
