#!/bin/sh

ACTION="mila_to_flang"
INPUT="-"
OUTPUT="-"

show_help() {
	echo "Flang/Mila Transpiler"
	echo ""
	echo "USAGE: $0 [OPTIONS]"
	echo ""
	echo "OPTIONS:"
	echo "  -a, --action <ACTION>  What to do [default: mila_to_flang] [possible values mila_to_flang, flang_to_mila]"
	echo "  -i, --input <PATH>     Path to input, defaults to stdin"
	echo "  -o, --output <PATH>    Path to output, defaults to stdout"
	echo "  -h, --help             Print help information"
	exit 0
}

while [ "$#" -gt 0 ]; do
	case "$1" in
	-a=* | --action=*)
		ACTION="${1#*=}"
		;;
	-a | --action)
		ACTION="$2"
		shift
		;;

	-i=* | --input=*)
		INPUT="${1#*=}"
		;;
	-i | --input)
		INPUT="$2"
		shift
		;;

	-o | --output)
		OUTPUT="$2"
		shift
		;;
	-o=* | --output=*)
		OUTPUT="${1#*=}"
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

if [ "$ACTION" = "mila_to_flang" ]; then
	SED_SCRIPT='
        s/begin/\{/g;
        s/end;/};/g;
        s/end\.$/}\./g;
        s/end/}/g;
        s/:[[:space:]]*integer/: Int/g;
        s/:[[:space:]]*real/: Real/g;
        s/:[[:space:]]*boolean/: Bool/g;
        s/of[[:space:]]*integer/of Int/g;
        s/of[[:space:]]*real/of Real/g;
        s/of[[:space:]]*boolean/of Bool/g;
        s/function/fn/g;
        s/procedure/proc/g;
        s/div/\//g;
        s/mod/%/g;
        s/<>/!=/g;
        s/\$([0-9a-fA-F]+)/16#\1/g;
        s/&([0-7]+)/8#\1/g;
    '
elif [ "$ACTION" = "flang_to_mila" ]; then
	SED_SCRIPT='
        s/\{/begin/g;
        s/\};/end;/g;
        s/\}\.$/end\./g;
        s/\}/end/g;
        s/:[[:space:]]*Int/: integer/g;
        s/:[[:space:]]*Real/: real/g;
        s/:[[:space:]]*Bool/: boolean/g;
        s/of[[:space:]]*Int/of integer/g;
        s/of[[:space:]]*Real/of real/g;
        s/of[[:space:]]*Bool/of boolean/g;
        s/fn/function/g;
        s/proc/procedure/g;
        s/\// div /g;
        s/%/ mod /g;
        s/!=/<>/g;
        s/16#([0-9a-fA-F]+)/$\1/g;
        s/8#([0-7]+)/\& \1/g;
    '
else
	echo "Error: Invalid action '$ACTION'. Use 'mila_to_flang' or 'flang_to_mila'." >&2
	exit 1
fi

if [ "$INPUT" = "-" ]; then
	sed -E "$SED_SCRIPT"
elif [ "$OUTPUT" = "-" ]; then
	sed -E "$SED_SCRIPT" "$INPUT"
else
	sed -E "$SED_SCRIPT" "$INPUT" >"$OUTPUT"
fi
