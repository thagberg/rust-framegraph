#version 450
layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 uv;

layout(set = 0, binding = 0) uniform Model {
    mat4 model;
    mat4 view;
    mat4 proj;
} model;

out gl_PerVertex {
    vec4 gl_Position;
};

layout(location=0) out struct {
    vec4 color;
    vec3 normal;
    vec2 uv;
} Out;

void main() {
    Out.uv = uv;
    Out.normal = normal;
    Out.color = vec4(1.0, 0.0, 0.0, 1.0);
    gl_Position = model.proj * model.view * model.model * vec4(position, 1.0);
}
