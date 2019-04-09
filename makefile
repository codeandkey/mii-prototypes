CC      = gcc
CFLAGS  = -std=gnu99 -Wall -Werror -Wno-unused-value -g
LDFLAGS = -lsqlite3

OUTPUT = bin/lmc

SOURCES = lmc.c
OBJECTS = $(SOURCES:.c=.o)

all: $(OUTPUT)

$(OUTPUT): $(OBJECTS)
	mkdir -p bin
	$(CC) $^ $(LDFLAGS) -o $@

%.o: %.c
	$(CC) $(CFLAGS) -c $< -o $@

clean:
	rm -f $(OUTPUT) $(OBJECTS)
