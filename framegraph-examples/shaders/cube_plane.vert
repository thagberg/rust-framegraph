#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec3 color;

layout(set = 0, binding = 0) uniform SceneUniforms {
    mat4 view;
    mat4 proj;
    vec4 light_pos;
    vec4 view_pos;
    vec4 light_dir;
    mat4 light_space_matrix;
    float spotlight_angle;
} scene;

layout(set = 0, binding = 1) uniform ModelUniforms {
    mat4 model;
} model_obj;

out gl_PerVertex {
    vec4 gl_Position;
};

layout(location = 0) out struct {
    vec3 frag_pos;
    vec3 normal;
    vec3 color;
    vec4 light_space_pos;
} Out;

void main() {
    vec4 world_pos = model_obj.model * vec4(position, 1.0);
    Out.frag_pos = world_pos.xyz;
    Out.normal = normalize(mat3(transpose(inverse(model_obj.model))) * normal);
    Out.color = color;
    Out.light_space_pos = scene.light_space_matrix * world_pos;
    
    gl_Position = scene.proj * scene.view * world_pos;
}
