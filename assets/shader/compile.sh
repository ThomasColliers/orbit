#!/bin/sh
glslc -o atmosphere.vert.spv atmosphere.vert
glslc -o atmosphere.frag.spv atmosphere.frag
glslc -o sun.vert.spv sun.vert
glslc -o sun.frag.spv sun.frag
glslc -o fsquad.vert.spv fsquad.vert
glslc -o fxaa.frag.spv fxaa.frag
glslc -o tonemap.frag.spv tonemap.frag