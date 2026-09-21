struct VSInput {
  float2 position;
  float3 color;
};

struct VSOutput {
  float4 pos : SV_Position;
  float3 color;
};

[shader("vertex")]
VSOutput vertMain(VSInput input) {
  VSOutput output;
  output.pos = float4(input.position, 0.0, 1.0);
  output.color = input.color;
  return output;
}

[shader("fragment")]
float4 fragMain(VSOutput input) : SV_TARGET { return float4(input.color, 1.0); }
