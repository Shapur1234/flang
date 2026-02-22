#!/bin/sh

INPUT="-"
OUTPUT="a.out"

show_help() {
	echo "x86-64 Assembly Build Wrapper"
	echo ""
	echo "USAGE: $0 [OPTIONS]"
	echo ""
	echo "OPTIONS:"
	echo "  -i, --input <PATH>      Path to .asm file, defaults to stdin"
	echo "  -o, --output <PATH>     Path to output binary, defaults to a.out"
	echo "  -h, --help              Print help information"
	exit 0
}

while [ "$#" -gt 0 ]; do
	case "$1" in
	-i=* | --input=*)
		INPUT="${1#*=}"
		;;
	-i | --input)
		INPUT="$2"
		shift
		;;
	-o=* | --output=*)
		OUTPUT="${1#*=}"
		;;
	-o | --output)
		OUTPUT="$2"
		shift
		;;
	-h | --help)
		show_help
		;;
	*)
		echo "Error: Unknown parameter '$1'" >&2
		exit 1
		;;
	esac
	shift
done

TMP_DIR=$(mktemp -d)
trap 'rm -r "$TMP_DIR" 2>/dev/null' EXIT

SRC_FILE="$TMP_DIR/source.asm"
OBJ_FILE="$TMP_DIR/output.o"

if [ "$INPUT" = "-" ]; then
	cat >"$SRC_FILE"
else
	if [ ! -f "$INPUT" ]; then
		echo "Input file '$INPUT' not found" >&2
		exit 1
	fi
	cp "$INPUT" "$SRC_FILE"
fi

if ! nasm -f elf64 "$SRC_FILE" -o "$OBJ_FILE"; then
	echo "Assembly failed" >&2
	exit 1
fi

if ! ld "$OBJ_FILE" -o "$OUTPUT"; then
	echo "Linking failed" >&2
	exit 1
fi

if ! strip -s "$OUTPUT"; then
	echo "Stripping failed" >&2
	exit 1
fi
