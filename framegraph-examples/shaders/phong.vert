#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;

layout(set = 0, binding = 0) uniform PhongUniforms {
    mat4 model;
    mat4 view;
    mat4 proj;
    vec4 light_pos;
    vec4 view_pos;
} uniforms;

out gl_PerVertex {
    vec4 gl_Position;
};

layout(location = 0) out struct {
    vec3 frag_pos;
    vec3 normal;
    vec3 light_pos;
    vec3 view_pos;
} Out;

void main() {
    vec4 world_pos = uniforms.model * vec4(position, 1.0);
    Out.frag_pos = world_pos.xyz;
    Out.normal = mat3(transpose(inverse(uniforms.model))) * normal;
    Out.light_pos = uniforms.light_pos.xyz;
    Out.view_pos = uniforms.view_pos.xyz;
    
    gl_Position = uniforms.proj * uniforms.view * world_pos;
}
