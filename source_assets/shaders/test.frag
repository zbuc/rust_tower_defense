#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) out vec4 outColor;
layout(location = 0) in vec3 fragColor;

// The triangle that is formed by the positions from the vertex
// shader fills an area on the screen with fragments. The fragment
// shader is invoked on these fragments to produce a color and depth
// for the framebuffer (or framebuffers). A simple fragment shader
// that outputs the color red for the entire triangle looks like this:
// https://vulkan-tutorial.com/Drawing_a_triangle/Graphics_pipeline_basics/Shader_modules

// The main function is called for every fragment just like the vertex
// shader main function is called for every vertex. Colors in GLSL are
// 4-component vectors with the R, G, B and alpha channels within the [0, 1]
// range. Unlike gl_Position in the vertex shader, there is no built-in
// variable to output a color for the current fragment. You have to specify
// your own output variable for each framebuffer where the layout(location = 0)
// modifier specifies the index of the framebuffer. The color red is written
// to this outColor variable that is linked to the first (and only) framebuffer
// at index 0.
void main() {
    outColor = vec4(fragColor, 1.0);
}
