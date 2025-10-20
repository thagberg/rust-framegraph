#version 450

layout(location = 0) out vec4 fragColor;

layout(location = 0) in struct {
    vec3 frag_pos;
    vec3 normal;
    vec3 light_pos;
    vec3 view_pos;
} In;

void main() {
    // Material properties
    vec3 object_color = vec3(0.8, 0.2, 0.3);
    vec3 light_color = vec3(1.0, 1.0, 1.0);
    
    // Ambient
    float ambient_strength = 0.1;
    vec3 ambient = ambient_strength * light_color;
    
    // Diffuse
    vec3 norm = normalize(In.normal);
    vec3 light_dir = normalize(In.light_pos - In.frag_pos);
    float diff = max(dot(norm, light_dir), 0.0);
    vec3 diffuse = diff * light_color;
    
    // Specular
    float specular_strength = 0.5;
    vec3 view_dir = normalize(In.view_pos - In.frag_pos);
    vec3 reflect_dir = reflect(-light_dir, norm);
    float spec = pow(max(dot(view_dir, reflect_dir), 0.0), 32.0);
    vec3 specular = specular_strength * spec * light_color;
    
    vec3 result = (ambient + diffuse + specular) * object_color;
    fragColor = vec4(result, 1.0);
}
