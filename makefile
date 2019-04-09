CC      = gcc
CFLAGS  = -std=gnu99 -Wall -Werror -Wno-unused-value -g
LDFLAGS = -lsqlite3

OUTPUT = lmc

SOURCES = lmc.c
OBJECTS = $(SOURCES:.c=.o)

all: $(OUTPUT)

$(OUTPUT): $(OBJECTS)
	$(CC) $^ $(LDFLAGS) -o $@

%.o: %.c
	$(CC) $(CFLAGS) -c $< -o $@

clean:
	rm -f $(OUTPUT) $(OBJECTS)
