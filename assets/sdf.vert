#version 450
layout(location = 0) out uint panel_idx;
layout(location = 1) out vec4 world_pos;
layout(set = 0, binding = 0) uniform Camera {
       mat4 ViewProj;
};
layout(set = 1, binding = 3) uniform SDFFunctions_panel_width
{
    uint panel_width;
};
layout(set = 1, binding = 4) uniform SDFFunctions_panel_height
{
    uint panel_height;
};
void main() {
    float tw = 1.0/panel_width;
    float th = 1.0/panel_height;
    panel_idx = gl_VertexIndex / 6;
    float tx = float(panel_idx % panel_width) * tw;
    float ty = float(panel_idx / panel_height) * th;
    uint i = gl_VertexIndex % 6;
    float x = float(((i + 2) / 3)%2);
    float y = float(((i + 1) / 3)%2);
    x = tx + x*tw;
    y = ty + y*th;
    gl_Position = vec4(-1.0f + x*2.0f, -1.0f+y*2.0f, 0.0f, 1.0f);
    world_pos = inverse(ViewProj) * gl_Position;
}
