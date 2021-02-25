#version 450
struct SDFObject {
    vec3 position;
    float rotation;
    float function_type;
    float a; float b;
    float noise_weight;
    vec4 color;
};

layout(location = 0) in flat uint panel_idx;
layout(location = 1) in vec4 world_pos;
layout(location = 0) out vec4 o_Target;
layout(set = 0, binding = 0) uniform Camera {
       mat4 ViewProj;
};
layout(set = 1, binding = 0) buffer SDFFunctions_functions
{
    SDFObject[] functions;
};
layout(set = 1, binding = 1) buffer SDFFunctions_tile_function_count
{
    uint[] tile_function_count;
};
layout(set = 1, binding = 2) buffer SDFFunctions_function_indices
{
    uint[] function_indices;
};

// Noise functions from https://www.iquilezles.org/
float hash(vec3 p)
{
    p  = 50.0*fract( p*0.3183099 + vec3(0.71,0.113,0.419));
    return -1.0+2.0*fract( p.x*p.y*p.z*(p.x+p.y+p.z) );
}

 vec4 noised( in vec3 x )
 {
    vec3 p = floor(x);
    vec3 w = fract(x);

    vec3 u = w*w*w*(w*(w*6.0-15.0)+10.0);
    vec3 du = 30.0*w*w*(w*(w-2.0)+1.0);

    float a = hash( p+vec3(0,0,0) );
    float b = hash( p+vec3(1,0,0) );
    float c = hash( p+vec3(0,1,0) );
    float d = hash( p+vec3(1,1,0) );
    float e = hash( p+vec3(0,0,1) );
    float f = hash( p+vec3(1,0,1) );
    float g = hash( p+vec3(0,1,1) );
    float h = hash( p+vec3(1,1,1) );

    float k0 =   a;
    float k1 =   b - a;
    float k2 =   c - a;
    float k3 =   e - a;
    float k4 =   a - b - c + d;
    float k5 =   a - c - e + g;
    float k6 =   a - b - e + f;
    float k7 = - a + b + c - d + e - f - g + h;

    return vec4( -1.0+2.0*(k0 + k1*u.x + k2*u.y + k3*u.z + k4*u.x*u.y + k5*u.y*u.z + k6*u.z*u.x + k7*u.x*u.y*u.z),
                 2.0* du * vec3( k1 + k4*u.y + k6*u.z + k7*u.y*u.z,
                                 k2 + k5*u.z + k4*u.x + k7*u.z*u.x,
                                 k3 + k6*u.x + k5*u.y + k7*u.x*u.y ) );
}

float fbm( in vec3 x, in float H )
{
    float G = exp2(-H);
    float f = 1.0;
    float a = 1.0;
    float t = 0.0;
    for( int i=0; i<3; i++ )
    {
        t += a*noised(f*x).x;
        f *= 2.0;
        a *= G;
    }
    return t;
}

mat2 rotate(float a){
    return mat2(
        cos(a), -sin(a),
        sin(a), cos(a)
    );
}

vec3 calc_color(float base_d, vec3 base_c) {
    vec3 result = vec3(0.0, 0.0, 0.0);
    uint count = 0;
    for(float dx = -0.35; dx < 0.35; dx += 0.05) {
        count += 1;
        float d = base_d + dx;
        if(d < 0.0) {
            result += base_c.rgb;
        }
    }
    return result/float(count);
}

void main() {
    vec4 pos = world_pos;
    vec4 pos_noise = vec4(fbm(world_pos.xyz / 10.0, 1000.0), fbm(world_pos.yxz / 10.0, 1000.0), 0.0, 0.0);
    uint function_count = tile_function_count[panel_idx*2];
    uint function_offset = tile_function_count[panel_idx*2+1];

    float min_d = 1.0/0.0;
    vec4 min_c = vec4(0.0,0.0,0.0,1.0);

    for(uint i = 0; i < function_count; i++) {
        SDFObject f = functions[function_indices[function_offset + i]];
        vec4 inner_pos = pos + pos_noise*f.noise_weight;

        float d = 1.0/0.0;
        vec2 dv = inner_pos.xy-f.position.xy;
        vec2 p = rotate(f.rotation)*(dv);
        if(f.function_type == 0.0) {
            d = length(dv) - f.a;
        } else if(f.function_type == 1.0) {
            vec2 dd = abs(p.xy)-vec2(f.a,f.b);
            d = length(max(dd,0.0)) + min(max(dd.x,dd.y),0.0);
        } else if(f.function_type == 2.0) {
            p.x = abs(p.x);
            vec2 q = vec2(f.a, f.b);
            vec2 a = p - q*clamp( dot(p,q)/dot(q,q), 0.0, 1.0 );
            vec2 b = p - q*vec2( clamp( p.x/q.x, 0.0, 1.0 ), 1.0 );
            float s = -sign( q.y );
            vec2 dd = min( vec2( dot(a,a), s*(p.x*q.y-p.y*q.x) ),
            vec2( dot(b,b), s*(p.y-q.y)  ));
            d = -sqrt(dd.x)*sign(dd.y);
        }

        if(d<min_d) {
            min_d = d;
            min_c = f.color;
        }
    }

    vec3 final_c = calc_color(min_d, min_c.rgb);
    o_Target = vec4(final_c, 1.0);
}
