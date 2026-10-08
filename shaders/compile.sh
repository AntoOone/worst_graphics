#!/bin/sh

SLANGC_FLAGS="-target spirv -profile spirv_1_4 -emit-spirv-directly -fvk-use-entrypoint-name -entry vertMain -entry fragMain"

set -eu

if [ "$#" -ne 1 ]; then
	echo "Usage: $0 <fichier.slang>" >&2
	exit 1
fi

input=$1

case "$input" in
*.slang)
	output="${input%.slang}.spv"
	cmd="slangc $input $SLANGC_FLAGS -o $output"
	eval "$cmd"
	;;
*)
	echo "Erreur : '$input' n'a pas l'extension .slang" >&2
	exit 1
	;;
esac
