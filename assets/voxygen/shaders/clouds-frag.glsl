#version 330 core

#include <constants.glsl>

#define LIGHTING_TYPE (LIGHTING_TYPE_TRANSMISSION | LIGHTING_TYPE_REFLECTION)

#define LIGHTING_REFLECTION_KIND LIGHTING_REFLECTION_KIND_SPECULAR

#if (FLUID_MODE == FLUID_MODE_CHEAP)
#define LIGHTING_TRANSPORT_MODE LIGHTING_TRANSPORT_MODE_IMPORTANCE
#elif (FLUID_MODE == FLUID_MODE_SHINY)
#define LIGHTING_TRANSPORT_MODE LIGHTING_TRANSPORT_MODE_RADIANCE
#endif

#define LIGHTING_DISTRIBUTION_SCHEME LIGHTING_DISTRIBUTION_SCHEME_MICROFACET

#define LIGHTING_DISTRIBUTION LIGHTING_DISTRIBUTION_BECKMANN

#include <globals.glsl>
// Note: The sampler uniform is declared here because it differs for MSAA
#include <anti-aliasing.glsl>
#include <srgb.glsl>
#include <cloud.glsl>

uniform sampler2D src_depth;

in vec2 f_pos;

layout (std140)
uniform u_locals {
    mat4 proj_mat_inv;
    mat4 view_mat_inv;
};

out vec4 tgt_color;

float depth_at(vec2 uv) {
    float buf_depth = texture(src_depth, uv).x;
    vec4 clip_space = vec4(uv * 2.0 - 1.0, buf_depth, 1.0);
    vec4 view_space = proj_mat_inv * clip_space;
    view_space /= view_space.w;
    return -view_space.z;
}

vec3 wpos_at(vec2 uv) {
    float buf_depth = texture(src_depth, uv).x * 2.0 - 1.0;
    mat4 inv = view_mat_inv * proj_mat_inv;//inverse(all_mat);
    vec4 clip_space = vec4(uv * 2.0 - 1.0, buf_depth, 1.0);
    vec4 view_space = inv * clip_space;
    view_space /= view_space.w;
    if (buf_depth == 1.0) {
        vec3 direction = normalize(view_space.xyz);
        return direction.xyz * 100000.0 + cam_pos.xyz;
    } else {
        return view_space.xyz;
    }
}

void main() {
    vec2 uv = (f_pos + 1.0) * 0.5;

    vec4 color = texture(src_color, uv);

    // Apply clouds to `aa_color`
    #if (CLOUD_MODE != CLOUD_MODE_NONE)
        vec3 wpos = wpos_at(uv);
        float dist = distance(wpos, cam_pos.xyz);
        vec3 dir = (wpos - cam_pos.xyz) / dist;

        color.rgb = get_cloud_color(color.rgb, dir, cam_pos.xyz, time_of_day.x, dist, 1.0);
    #endif

    tgt_color = vec4(color.rgb, 1);
}
