#version 450

layout(location = 0) in vec2 tex_coords;
layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform texture2D tex;
layout(set = 0, binding = 1) uniform sampler tex_sampler;
layout(set = 0, binding = 2) uniform Uniforms {
    uint iteration;
    float width;
    float height;
};

void main() {
    vec3 color = texture(sampler2D(tex, tex_sampler), tex_coords).rgb;
    float color_val = (color.r + color.g + color.b) / 3.0;

    int iteration_parity = int(iteration % 2);
    int pixel_x_parity = int(floor(tex_coords.x * width)) % 2;

    int a = (pixel_x_parity * 2 - 1) * (iteration_parity * 2 - 1);

    vec2 other_tex_coords = clamp(tex_coords + vec2(a / width, 0.0), vec2(0.0), vec2(1.0));
    vec3 other_color = texture(sampler2D(tex, tex_sampler), other_tex_coords).rgb;
    float other_color_val = (other_color.r + other_color.g + other_color.b) / 3.0;

    float threshold = 0.05;
    if (color_val > threshold && other_color_val > threshold) {
        if (a > 0.0 && color_val > other_color_val) {
            color = other_color;
        } else if (a < 0.0 && other_color_val > color_val) {
            color = other_color;
        }
    }

    f_color = vec4(color, 1.0);
}
