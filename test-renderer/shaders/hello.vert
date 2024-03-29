#version 450

layout(location = 0) out vec3 fragColor;

layout(set = 0, binding = 0) uniform Offset {
    vec3 offset;
} ubo;

vec2 positions[3] = vec2[](
    vec2(0.0, -0.5),
    vec2(0.5, 0.5),
    vec2(-0.5, 0.5)
);

vec3 colors[3] = vec3[](
    vec3(1.0, 0.0, 0.0),
    vec3(0.0, 1.0, 0.0),
    vec3(0.0, 0.0, 1.0)
);

void main() {
    gl_Position = vec4(positions[gl_VertexIndex], 0.0, 0.5) + vec4(ubo.offset, 0.0);
    fragColor = colors[gl_VertexIndex];
}