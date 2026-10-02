struct VSInput {
  float2 position;
  float3 color;
  float2 pos;
};

struct UniformBuffer {
  float4x4 model;
  float4x4 view;
  float4x4 proj;
};
ConstantBuffer<UniformBuffer> ubo;

struct VSOutput {
  float4 pos : SV_Position;
  float3 color;
};

[shader("vertex")]
VSOutput vertMain(VSInput input) {
  VSOutput output;
  output.pos =
      mul(ubo.proj,
          mul(ubo.view,
              mul(ubo.model, float4(input.position + input.pos, 0.0, 1.0))));
  output.color = input.color;
  return output;
}

[shader("fragment")]
float4 fragMain(VSOutput input) : SV_TARGET { return float4(input.color, 1.0); }
