#version 450

layout(location = 0) out vec4 fragColor;

layout(location = 0) in struct {
    vec4 color;
    vec2 uv;
    vec3 normal;
} In;

void main() {
    fragColor = In.color;
}