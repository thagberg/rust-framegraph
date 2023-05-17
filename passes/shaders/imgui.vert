#version 450
layout(location = 0) in vec2 aPos;
layout(location = 1) in vec2 aUV;
layout(location = 2) in vec4 aColor;

out gl_PerVertex {
    vec4 gl_Position;
};

layout(set = 0, binding = 0) uniform Display {
    vec2 scale;
    vec2 pos;
} display;

layout(location = 0) out struct {
    vec4 Color;
    vec2 UV;
} Out;

void main()
{
    Out.Color = aColor;
    Out.UV = aUV;
    gl_Position = vec4(aPos * display.scale * display.pos, 0, 1);
}