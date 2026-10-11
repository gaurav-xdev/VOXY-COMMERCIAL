# NVIDIA NIM Provider Usage & Verification

## 1. Provider Discovery
- **Provider Adapter:** `voxy-openai` (`OpenAIConfig::nvidia_nim`)
- **API Base URL:** `https://integrate.api.nvidia.com`
- **Configured Environment Variable:** `VOXY_API_KEYS_NVIDIA` (with `NVIDIA_API_KEY` fallback)
- **Configured Model Identifier:** `nvidia/nemotron-3-ultra-550b-a55b` (discovered in `.env`)
- **Credentials Protection:** Zero credentials logged, exposed, or written to reports.

## 2. API Health & Connectivity Check
- **Endpoint:** `GET https://integrate.api.nvidia.com/v1/models`
- **HTTP Status:** `200 OK`
- **Total Discovered Models:** 80 models available
- **Model Availability:** `nvidia/nemotron-3-ultra-550b-a55b` confirmed present on the endpoint.

## 3. Bounded Inference Execution Verification
- **Request Type:** Single bounded test prompt (`max_tokens: 32`, `temperature: 0.1`)
- **HTTP Status:** `200 OK`
- **Round-Trip Latency:** 8.956 seconds
- **Token Accounting:**
  - `prompt_tokens`: 33
  - `completion_tokens`: 32
  - `total_tokens`: 65
- **Verified Response:** Model successfully returned coherent text acknowledging coding harness prompt without rate-limits or timeouts.
- **Budget Compliance:** Exactly 1 inference request executed. Zero token burn.
