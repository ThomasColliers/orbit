#version 450

#include "header/math.frag"

#include "header/fxaa.frag"

layout(std140, set = 0, binding = 0) uniform FXAAUniformArgs {
    uniform float screen_width;
    uniform float screen_height;
};

layout(set = 1, binding = 0) uniform sampler2D color;

layout(location = 0) in VertexData {
    vec3 position;
    vec2 tex_coord;
} vertex;

layout(location = 0) out vec4 out_color;

void main(){
    //vec4 input_color = texture(color, vertex.tex_coord);
    // simply output the texture coordinate for now
    //out_color = input_color;
    out_color = vec4(vertex.tex_coord, 0.0, 1.0);
}