#version 450

layout(location = 0) in vec3 fragColor;
layout(location = 0) out vec4 outColor;

layout(set = 0, binding = 1) uniform sampler2D tex;
layout(set = 1, binding = 0) uniform sampler2D tex2;

void main() {
    //outColor = vec4(fragColor, 1.0);
    outColor = texture(tex, vec2(0.0, 0.0));
}
