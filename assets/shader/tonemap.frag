#version 450

layout(std140, set = 0, binding = 0) uniform TonemapUniformArgs {
    uniform bool enabled;
    uniform float exposure;
};

layout(set = 0, binding = 1) uniform sampler2D color;

layout(location = 0) in VertexData {
    vec3 position;
    vec2 tex_coord;
} vertex;

layout(location = 0) out vec4 out_color;

void main(){
    if(!enabled){
        out_color = texture(color, vertex.tex_coord);
        return;
    }
    
    const float gamma = 2.2;
    vec3 hdr = texture(color, vertex.tex_coord).rgb;
    // exposure tone mapping
    vec3 mapped = vec3(1.0) - exp(-hdr * exposure);
    // gamma correction
    mapped = pow(mapped, vec3(1.0 / gamma));
    out_color = vec4(mapped, 1.0);
}