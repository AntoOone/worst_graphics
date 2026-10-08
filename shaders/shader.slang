struct Vertex {
  float3 position;
  float3 color;
}

struct Instance {
  float4x4 local_to_world;
}

struct VSInput {
  Vertex v;
  Instance i;
};

struct UniformBuffer {
  float4x4 world_to_screen;
};
ConstantBuffer<UniformBuffer> ubo;

struct VSOutput {
  float4 position : SV_Position;
  float3 color;
};

[shader("vertex")]
VSOutput vertMain(VSInput input) {
  float4 world_pos = mul(input.i.local_to_world, float4(input.v.position, 1));
  VSOutput output;
  output.position = mul(ubo.world_to_screen, world_pos);
  output.color = input.v.color;
  return output;
}

[shader("fragment")]
float4 fragMain(VSOutput input) : SV_TARGET { return float4(input.color, 1.0); }
