#version 450
#extension GL_ARB_separate_shader_objects : enable

// Normally these coordinates would be stored in a vertex buffer,
// but creating a vertex buffer in Vulkan and filling it with data
// is not trivial. Therefore I've decided to postpone that until
// after we've had the satisfaction of seeing a triangle pop up on
// the screen. We're going to do something a little unorthodox in
// the meanwhile: include the coordinates directly inside the vertex
// shader. The code looks like this:
// https://vulkan-tutorial.com/Drawing_a_triangle/Graphics_pipeline_basics/Shader_modules

layout(location = 0) out vec3 fragColor;

vec2 positions[3] = vec2[](
    vec2(0.0, -0.5),
    vec2(0.5, 0.5),
    vec2(-0.5, 0.5)
);

vec3 colors[3] = vec3[](
    vec3(1.0, 0.0, 0.0),
    vec3(0.0, 1.0, 0.0),
    vec3(0.0, 0.0, 1.0)
);

void main() {
    gl_Position = vec4(positions[gl_VertexIndex], 0.0, 1.0);
    fragColor = colors[gl_VertexIndex];
}
