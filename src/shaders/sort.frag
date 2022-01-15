#version 450

layout(location = 0) in vec2 tex_coords;
layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform texture2D frame;
layout(set = 0, binding = 1) uniform texture2D field;
layout(set = 0, binding = 2) uniform sampler tex_sampler;
layout(set = 0, binding = 3) uniform Uniforms {
    uint iteration;
    float width;
    float height;
};

// uses odd-even algorithm instructions from the field to sort the frame 
void main() {
    // get the frames pixel color
    vec3 color = texture(sampler2D(frame, tex_sampler), tex_coords).rgb;
    float color_val = (color.r + color.g + color.b) / 3.0;
    
    // read a vector from the generated field
    vec3 data = texture(sampler2D(field, tex_sampler), tex_coords).xyz;

    // if directions are zero skip
    if (data == vec3(0.0)) {
        f_color = vec4(color, 1.0);
        return;
    }

    // extract insructions from the pixel
    vec2 vector = data.xy;
    float direction = sign(data.z);
    float threshold = abs(data.z < 0.0 ? data.z + 1.0 : data.z - 1.0);// fract(data.z);
    float a = sign(vector.x * 2.0 + vector.y) * 2.0 - 1.0;

    // get the other pixel's coords
    vec2 resolution = vec2(width, height);
    vec2 other_tex_coords = clamp(tex_coords + vector / resolution, vec2(0.0), vec2(1.0));
    if (other_tex_coords == tex_coords) {
        f_color = vec4(color, 1.0);
        return;
    }

    // get other pixel color
    vec3 other_color = texture(sampler2D(frame, tex_sampler), other_tex_coords).rgb;
    float other_color_val = (other_color.r + other_color.g + other_color.b) / 3.0;

    // if both color vals are not above the threshold skip
    if (!(color_val > threshold && other_color_val > threshold)) {
        f_color = vec4(color, 1.0);
        return;
    }

    // comparing the 'next' or 'previous'
    if (a > 0.0) {
        // comparing next, check direction
        if (direction < 0.0) {
            // sort in reverse
            if (color_val < other_color_val) {
                color = other_color;
            }
        } else {
            // sort normal
            if (color_val > other_color_val) {
                color = other_color;
            }
        }
    } else if (a < 0.0) {
        // comparing previous, check direction
        if (direction < 0.0) {
            // sort in reverse
            if (color_val > other_color_val) {
                color = other_color;
            }
        } else {
            // sort normal
            if (color_val < other_color_val) {
                color = other_color;
            }
        }
    }

    f_color = vec4(color, 1.0);
}
