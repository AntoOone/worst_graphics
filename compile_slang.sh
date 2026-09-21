#!/usr/bin/env sh

if ! [ -d target ]; then
	mkdir target
fi

slangc src/shader.slang -target spirv -profile spirv_1_4 -emit-spirv-directly -fvk-use-entrypoint-name -entry vertMain -entry fragMain -o target/shader.spv
