struct Uniforms {
    mvp: mat4x4<f32>,
    mv: mat4x4<f32>,
    normal: mat4x4<f32>,
    color: vec4<f32>,
    material_ambient: vec4<f32>,
    material_diffuse: vec4<f32>,
    material_specular: vec4<f32>,
    settings: vec4<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var material_texture: texture_2d<f32>;
@group(0) @binding(2) var material_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) texcoord: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) tangent: vec3<f32>,
    @location(4) parameter: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) view_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec3<f32>,
    @location(3) texcoord: vec2<f32>,
    @location(4) parameter: vec4<f32>,
};

@vertex
fn vertex_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    let position = vec4<f32>(input.position, 1.0);
    output.clip_position = uniforms.mvp * position;
    output.view_position = (uniforms.mv * position).xyz;
    output.normal = normalize((uniforms.normal * vec4<f32>(input.normal, 0.0)).xyz);
    let tangent = (uniforms.normal * vec4<f32>(input.tangent, 0.0)).xyz;
    if all(tangent == vec3<f32>(0.0)) {
        output.tangent = tangent;
    } else {
        output.tangent = normalize(tangent);
    }
    output.texcoord = input.texcoord;
    output.parameter = input.parameter;
    return output;
}

@fragment
fn fragment_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let mode = u32(uniforms.settings.z);
    var color: vec4<f32>;
    if mode == 0u {
        color = uniforms.color;
    } else {
        let sampled = textureSample(material_texture, material_sampler, input.texcoord);
        if mode == 1u {
            color = sampled;
        } else if mode == 3u {
            color = vec4<f32>(uniforms.color.rgb, sampled.r);
        } else {
            color = vec4<f32>(sampled.g * uniforms.color.rgb, sampled.r);
        }
        if color.a == 0.0 {
            discard;
        }
    }

    let light_ambient = vec3<f32>(0.73);
    let light_diffuse = vec3<f32>(0.60);
    let light_specular = vec3<f32>(0.70);
    let light_direction = vec3<f32>(-0.4531539381, 0.4226179123, 0.7848858833);
    let normal = normalize(input.normal);
    let eye = normalize(-input.view_position);
    let light_dot = max(dot(light_direction, normal), 0.1);
    let ambient = light_ambient * uniforms.material_ambient.rgb;
    let diffuse = light_diffuse * uniforms.material_diffuse.rgb * light_dot;
    let reflected = reflect(-light_direction, normal);
    let blinn = pow(max(dot(reflected, eye), 0.0), uniforms.settings.x);
    var reflection = blinn;
    var strength = 1.0;
    if uniforms.settings.y != 0.0 {
        let dot_lt = dot(light_direction, input.tangent);
        let dot_vt = dot(eye, input.tangent);
        let dot_ln = sqrt(1.0 - dot_lt * dot_lt);
        let dot_vr = dot_ln * sqrt(1.0 - dot_vt * dot_vt) - dot_lt * dot_vt;
        let anisotropic = pow(max(0.0, dot_vr), uniforms.settings.x);
        reflection = mix(anisotropic, blinn, input.parameter.r);
        strength = input.parameter.g;
    }
    let specular = light_specular * uniforms.material_specular.rgb * reflection * strength;
    let rim = vec3<f32>(uniforms.settings.w) * pow(input.parameter.a * (1.0 - abs(normal.z)), 2.0);
    color = vec4<f32>((ambient + diffuse) * color.rgb + specular + rim, color.a);
    return color;
}
