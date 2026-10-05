# Generates the micropython_embed/ source package (MicroPython "embed" port).
MICROPYTHON_TOP ?= ../../../../micropython

# Make the qstr/module scanner see our C module.
SRC_QSTR += modhost.c $(MICROPYTHON_TOP)/extmod/modjson.c

include $(MICROPYTHON_TOP)/ports/embed/embed.mk
