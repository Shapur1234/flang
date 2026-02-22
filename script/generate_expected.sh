#!/bin/sh

SAMPLES_DIR="./tests/samples"
EXPECTED_BASE="./tests/expected"
ACTIONS="tokens derivation ast opt-ast stack asm-amd64"

echo "Building flang..."
nix build .# || {
	echo "Build failed" >&2
	exit 2
}

FLANG_BIN="./result/bin/flang"
if test ! -x "${FLANG_BIN}"; then
	echo "flang not found at \"${FLANG_BIN}\"" >&2
	return 3
fi

echo "Using flang at \"${FLANG_BIN}\""

for INPUT_FILE in "${SAMPLES_DIR}"/*; do
	[ -e "$INPUT_FILE" ] || continue

	BASENAME=$(basename "$INPUT_FILE")
	PROG_NAME="${BASENAME%.*}"
	PROG_DIR="${EXPECTED_BASE}/${PROG_NAME}"

	echo "Processing sample: ${BASENAME} -> ${PROG_DIR}"
	mkdir -p "${PROG_DIR}"

	for ACTION in $ACTIONS; do
		OUT_FILE="${PROG_DIR}/${ACTION}"

		"${FLANG_BIN}" --action="${ACTION}" --input="${INPUT_FILE}" --output="${OUT_FILE}" || {
			echo "Failed: ${BASENAME} with action ${ACTION}" >&2
			exit 3
		}
	done
done

echo "Generation complete."
