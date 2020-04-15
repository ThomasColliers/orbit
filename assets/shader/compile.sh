#!/bin/sh
glslc -o atmosphere.vert.spv atmosphere.vert
glslc -o atmosphere.frag.spv atmosphere.frag
glslc -o fxaa.vert.spv fxaa.vert
glslc -o fxaa.frag.spv fxaa.frag
