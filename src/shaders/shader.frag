// NOTE: This shader requires being manually compiled to SPIR-V in order to
// avoid having downstream users require building shaderc and compiling the
// shader themselves. If you update this shader, be sure to also re-compile it
// and update `frag.spv`. You can do so using `glslangValidator` with the
// following command: `glslangValidator -V shader.frag`

#version 450

layout(location = 0) in vec2 tex_coords;
layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform texture2D tex;
layout(set = 0, binding = 1) uniform sampler tex_sampler;
layout(set = 0, binding = 2) uniform Uniforms {
    float width;
    float height;
    uint iteration;
};

void main() {
    vec3 color = texture(sampler2D(tex, tex_sampler), tex_coords).rgb;
    float color_val = (color.r + color.g + color.b) / 3.0;

    vec2 resolution = vec2(width, height);
    ivec2 pixel_coord = ivec2(floor(tex_coords * resolution));
    int iteration_parity = int(iteration % 2);
    int pixel_x_parity = pixel_coord.x % 2;

    int a = (pixel_x_parity * 2 - 1) * (iteration_parity * 2 - 1);
    ivec2 other_pixel_coord = pixel_coord + ivec2(1, 0) * a;

    vec3 other_color = texture(sampler2D(tex, tex_sampler), vec2(other_pixel_coord) / resolution).rgb;
    float other_color_val = (other_color.r + other_color.g + other_color.b) / 3.0;

    if (color_val > other_color_val && a > 0) {
        color = other_color;
    } else if (color_val < other_color_val && a < 0) {
        color = other_color;
    }

    f_color = vec4(color, 1.0);
}
