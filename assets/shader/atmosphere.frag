#version 450

layout(location = 0) out vec4 out_color;

void main(){
    vec4 color = vec4(0.123,0.34,0.8,0.5);
    out_color = color;
}