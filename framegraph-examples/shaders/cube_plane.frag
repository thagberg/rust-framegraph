#version 450

layout(location = 0) out vec4 fragColor;

layout(set = 0, binding = 0) uniform SceneUniforms {
    mat4 view;
    mat4 proj;
    vec4 light_pos;
    vec4 view_pos;
    vec4 light_dir;
    float spotlight_angle;
} scene;

layout(set = 0, binding = 2) uniform sampler2DShadow shadowMap;

layout(location = 0) in struct {
    vec3 frag_pos;
    vec3 normal;
    vec3 color;
} In;

void main() {
    // Material properties
    vec3 object_color = In.color;
    vec3 light_color = vec3(1.0, 1.0, 1.0);
    
    // Ambient
    float ambient_strength = 0.1;
    vec3 ambient = ambient_strength * light_color;
    
    // Spotlight logic
    vec3 light_to_frag = normalize(In.frag_pos - scene.light_pos.xyz);
    float theta = dot(light_to_frag, normalize(scene.light_dir.xyz));
    
    vec3 result;
    if (theta > scene.spotlight_angle) {
        // Diffuse
        vec3 norm = normalize(In.normal);
        vec3 light_dir = -light_to_frag;
        float diff = max(dot(norm, light_dir), 0.0);
        vec3 diffuse = diff * light_color;
        
        // Specular
        float specular_strength = 0.5;
        vec3 view_dir = normalize(scene.view_pos.xyz - In.frag_pos);
        vec3 reflect_dir = reflect(-light_dir, norm);
        float spec = pow(max(dot(view_dir, reflect_dir), 0.0), 32.0);
        vec3 specular = specular_strength * spec * light_color;
        
        // Simple smoothing at the edges
        float epsilon = 0.05;
        float intensity = clamp((theta - scene.spotlight_angle) / epsilon, 0.0, 1.0);
        
        result = (ambient + (diffuse + specular) * intensity) * object_color;
    } else {
        result = ambient * object_color;
    }

    fragColor = vec4(result, 1.0);
}
