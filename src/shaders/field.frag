#version 450

layout(location = 0) in vec2 tex_coords;
layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform Uniforms {
    uint iteration;
    float width;
    float height;
};

void main() {
    vec2 st = tex_coords;

    int iteration_parity = int(iteration % 2);
    int pixel_parity = int(floor(tex_coords.y * height)) % 2;

    int a = (pixel_parity * 2 - 1) * (iteration_parity * 2 - 1);

    f_color = vec4(0, a, 0, 1);
}
