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

    vec2 resolution = vec2(width, height);
    vec2 pixel_coord = floor(tex_coords * resolution);
    int iteration_parity = int(iteration % 2);
    int pixel_x_parity = int(pixel_coord.x) % 2;

    int a = (pixel_x_parity * 2 - 1) * (iteration_parity * 2 - 1);
    vec2 other_pixel_coord = pixel_coord + vec2(a, 0.0);

    vec2 other_tex_coords = clamp(other_pixel_coord / resolution, vec2(0.0), vec2(1.0));
    if (other_tex_coords != tex_coords) {
        vec3 other_color = texture(sampler2D(tex, tex_sampler), other_tex_coords).rgb;
        float other_color_val = (other_color.r + other_color.g + other_color.b) / 3.0;

        // if we are comparing with the next pixel and we are brighter, swap
        if (a > 0.0 && color_val > other_color_val) {
            color = other_color;
        } else 
        // if we are comparing with the previous pixel and are dimmer, swap
        if (a < 0.0 && other_color_val > color_val) {
            color = other_color;
        }
    }

    f_color = vec4(color, 1.0);
}
