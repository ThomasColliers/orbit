#version 450

#include "header/math.frag"

#include "header/environment.frag"

layout(location = 0) in VertexData {
    vec3 position;
    vec3 normal;
    vec3 tangent;
    float tang_handedness;
} vertex;

layout(location = 0) out vec4 out_color;

void main(){
    // the sun is simply bright..
    out_color = vec4(10.0,10.0,10.0,1.0);
}