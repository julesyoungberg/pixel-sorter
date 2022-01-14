#version 450

layout(location = 0) in vec2 tex_coords;
layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform Uniforms {
    uint iteration;
    float width;
    float height;
};

void main() {
    f_color = vec4(0.0);
}
