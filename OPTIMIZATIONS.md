# LLM Router Optimizations

This document outlines the performance, reliability, and security optimizations applied to the LLM Router.

## 1. Model and Policy Optimizations

### Model Assignments

- **Code Generation**: Updated to use `deepseek/deepseek-coder-v2` for better code generation capabilities
- **Open QA**: Now uses `nvidia/llama-3.3-nemotron-super-49b-v1` for enhanced reasoning capabilities
- **Other task types**: Appropriate models assigned based on task complexity and requirements

### Triton Model Configurations

- **Increased batch size** from 8 to 16 for better throughput
- **Added dynamic batching** with preferred batch sizes of 4, 8, and 16
- **FP16 precision mode** for faster inference with minimal accuracy loss
- **TensorRT acceleration** enabled for both task and complexity routers
- **CUDA Graph optimization** for repeated inference patterns
- **GPU profile selection** for optimized model execution

## 2. Router Controller Optimizations

### Performance Improvements

- **Connection pooling**: Increased pool size to 100 for better concurrent request handling
- **Request timeout**: Set to 180 seconds to accommodate larger inference tasks
- **Load balancing**: Implemented weighted random strategy for better distribution
- **Response caching**: Enabled with 1-hour TTL and 5000 entry capacity
- **Retry mechanism**: Configured with 3 retries and exponential backoff

### Reliability Enhancements

- **Circuit breaker pattern**: Enabled with failure threshold of 5 and 30-second reset timeout
- **Error handling**: Improved error handling and reporting throughout the system
- **Health checks**: Enhanced health monitoring for better system stability

### Security Improvements

- **Rate limiting**: Configured per-IP rate limiting at 50 requests/second with burst capability
- **Environment variable substitution**: Secret management through environment variables
- **JSON logging**: Enabled structured logging for better analysis

## 3. Triton Server Optimizations

### Configuration Improvements

- **HTTP thread count**: Increased to 8 for better request handling
- **Memory management**: Optimized GPU memory fraction and pinned memory allocation
- **Cache size**: Configured 1GB response cache for frequently requested inferences
- **Low-level settings**: TensorRT, CUDA, and PyTorch backend optimizations

### Metrics and Monitoring

- **GPU metrics**: Enabled detailed GPU utilization metrics
- **Metrics interval**: Set to 1 second for real-time monitoring
- **Verbose logging**: Configured appropriate logging levels for production use

## 4. Using the Optimized Router

### Starting with Optimized Configuration

```bash
# Start Triton server with optimized configuration
tritonserver --config-file=/path/to/triton_config.yaml

# Start the LLM Router
cd /app/llm-router
python -m src.test_router --port 8084
```

### Testing the Router

```python
from openai import OpenAI

client = OpenAI(
    api_key="your_api_key",
    base_url="http://localhost:8084/v1/"
)

response = client.chat.completions.create(
    messages=[{"role": "user", "content": "Write a Python function to sort a list"}],
    model="",  # Model will be selected by the router
    extra_body={
        "nim-llm-router": {
            "policy": "task_router",
            "routing_strategy": "triton"
        }
    }
)

print(response.choices[0].message.content)
```

## 5. Monitoring & Metrics

The optimized router exports metrics on port 9090, which can be scraped by Prometheus. Key metrics include:

- Request counts per policy and model
- Latency measurements
- Cache hit/miss ratios
- Circuit breaker status
- Token usage statistics

## 6. Future Improvements

- **Auto-scaling**: Implement automatic scaling based on request volume
- **A/B testing**: Support for model experimentation and evaluation
- **Custom policies**: Enable user-defined routing policies
- **Federated deployment**: Support for multi-region deployment with latency-based routing 