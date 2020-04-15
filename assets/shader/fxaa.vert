#version 450

layout(location = 0) in vec2 position;
layout(location = 1) in vec2 tex_coord;

layout(location = 0) out VertexData {
    vec3 position;
    vec2 tex_coord;
} vertex;

void main() {
    vec4 vertex_position = vec4(position, 1.0, 1.0);
    vertex.position = vertex_position.xyz;
    vertex.tex_coord = tex_coord;
    gl_Position = vertex_position;
}