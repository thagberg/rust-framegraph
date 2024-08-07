#version 450

layout(location = 0) out vec4 fragColor;

layout(location = 0) in struct {
    vec4 color;
    vec3 normal;
    vec2 uv;
} In;

layout(binding = 1) uniform sampler2D colorSampler;

void main() {
    fragColor = texture(colorSampler, In.uv);
}