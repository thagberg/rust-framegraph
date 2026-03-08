#version 450

layout(location = 0) in vec3 aPos;

layout(set = 0, binding = 0) uniform LightMVP {
    mat4 view;
    mat4 proj;
} light;

layout(push_constant) uniform Model {
    mat4 model;
} obj;

void main() {
    gl_Position = light.proj * light.view * obj.model * vec4(aPos, 1.0);
}
