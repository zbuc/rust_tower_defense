# Day 9

Thought about webassembly + webgl but there's currently no support, and only an
in-development branch. Opportunity to contribute, but a difficult distraction
from the game.

https://github.com/gfx-rs/gfx/pull/2554


Found the author, a gfx-rs dev, was working on converting an existing tutorial in Vulkan to
gfx-rs and the code looked much simpler to grok than the examples from gfx-rs itself.

https://github.com/grovesNL/gfx-hal-tutorial

https://vulkan-tutorial.com/

https://vulkan-tutorial.com/Overview

> You can query for properties like VRAM size and device capabilities to select desired devices, for example to prefer using dedicated graphics cards.

https://vulkan-tutorial.com/Drawing_a_triangle/Graphics_pipeline_basics


> Over the course of the next few chapters we'll be setting up a graphics pipeline that is configured to draw our first triangle. The graphics pipeline is the sequence of operations that take the vertices and textures of your meshes all the way to the pixels in the render targets. A simplified overview is displayed below:

https://vulkan-tutorial.com/images/vulkan_simplified_pipeline.svg

Vertex/Index Buffer -> Input Assembler -> Vertex Shader -> Tessellation -> Geometry Shader -> Rasterization -> Fragment Shader -> Color Blending -> Framebuffer

The Vulkan tutorial is very useful for learning how a graphics pipeline works.

Fullscreen mode: https://github.com/tomaka/winit/blob/master/examples/fullscreen.rs