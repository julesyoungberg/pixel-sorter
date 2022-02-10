#version 450

layout(location = 0) in vec2 tex_coords;
layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform Uniforms {
    uint iteration;
    float width;
    float height;
};

/**
*
* Generates a veector field to control the sorter.
* Encodes directions in the color channels:
* - r: x shift - one of [-1, 0, 1]
* - g: y shift - one of [-1, 0, 1]
* - b: must be nonzero
*   - fractional part represents the sort threshold. 
*   - the sign represents the sort direction.
*
* Rules (https://ciphrd.com/2020/04/08/pixel-sorting-on-shader-using-well-crafted-sorting-filters-glsl/):
*   1. For every vector of the vector field, located at (x, y) 
*      pointing to (x’, y’), there must be a vector located at (x’, y’) 
*      pointing to (x, y). In other words, texels that should be swapped 
*      should have opposite vectors on the vector field.
*   2. The vector field should at least have 2 states (A, B), where 
*      pairs of texels from state A should be different than the pairs 
*      from state B. This is required for the sort to be happening globally 
*      over time
*   3. Every component of the vectors from the vector field shouldn’t have a 
*      fractional part, so that we can be sure to land on another texel.
*   4. As described previously, to offer diversity and control to the vector 
*      field, the blue component can encode the direction of the sort and the 
*      alpha component can encode whether the sort is possible or not.
*
*
*/

vec3 horizontal(float direction) {
    int iteration_parity = int(iteration % 2);
    int pixel_parity = int(floor(tex_coords.x * width)) % 2;

    int a = (pixel_parity * 2 - 1) * (iteration_parity * 2 - 1);

    return vec3(a, 0, direction);
}

vec3 vertical(float direction) {
    int iteration_parity = int(iteration % 2);
    int pixel_parity = int(floor(tex_coords.y * height)) % 2;

    int a = (pixel_parity * 2 - 1) * (iteration_parity * 2 - 1);

    return vec3(0, a, direction);
}

vec3 diagonal(float direction) {
    int iteration_parity = int(iteration % 2);
    int pixel_parity = int(floor(tex_coords.x * width)) % 2;

    int a = (pixel_parity * 2 - 1) * (iteration_parity * 2 - 1);

    return vec3(a, a, direction);
}

vec3 mirror_diagonal(float direction) {
    vec2 st = tex_coords;
    float diff = st.x - 0.5;

    if (abs(diff) < 0.05) {
        return vec3(0);
    }

    int iteration_parity = int(iteration % 2);
    int pixel_parity = int(floor(tex_coords.y * height)) % 2;

    int a = (pixel_parity * 2 - 1) * (iteration_parity * 2 - 1);

    return vec3(a, a, sign(diff) * direction);
}

vec3 diagonals(float direction) {
    vec2 st = tex_coords;

    // top
    if (st.x > st.y && st.x < 1.0 - st.y) {
        return vertical(1.0 * direction);
    }

    // left
    if (st.y > st.x && st.y < 1.0 - st.x) {
        return horizontal(1.0 * direction);
    }

    // bottom
    if (st.y > st.x && 1.0 - st.x < st.y) {
        return vertical(-1.0 * direction);
    }
    
    // right
    if (st.x > st.y && 1.0 - st.y < st.x) {
        return horizontal(-1.0 * direction);
    }

    return vec3(0);
}

vec3 vertical_diagonal(float direction) {
    int iteration_parity = int(iteration % 2);
    int pixel_parity = int(floor(tex_coords.y * height)) % 2;

    int a = (pixel_parity * 2 - 1) * (iteration_parity * 2 - 1);

    return vec3(a, a, direction);
}

vec3 vertical_inverse(float direction) {
    int iteration_parity = int(iteration % 2);
    int pixel_parity = int(floor(tex_coords.y * height)) % 2;

    int a = (pixel_parity * 2 - 1) * (iteration_parity * 2 - 1);

    return vec3(a, a * -1.0, direction);
}

vec3 zig_zag(float direction) {
    float grid = 5.0;
    int y_parity = int(floor(tex_coords.y * grid)) % 2;
    
    if (y_parity > 0) {
        return vertical_inverse(1.0 * direction);
    }

    // return vertical_inverse(-1.0 * direction);
    return vertical_diagonal(-1.0 * direction);
}

void main() {
    vec3 color = vec3(0);

    // color = horizontal(1.0);
    // color = vertical(-1.0);
    color = diagonal(-1.01);
    // color = mirror_diagonal(1.001);
    // color = diagonals(1.001);
    // color = vertical_inverse(1.0);
    // color = zig_zag(1.001);

    f_color = vec4(color, 1.0);
}
