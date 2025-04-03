use hyper::body::{Body, Incoming};
use http_body_util::Full;
use bytes::Bytes;

pub async fn proxy(
    req: Request<Incoming>,
    config: &Config,
) -> Result<Response<Full<Bytes>>, Box<dyn std::error::Error>> {
    // ... existing code ...
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::body::Body;
    
    // ... existing code ...
    
    #[tokio::test]
    async fn test_proxy() {
        let config = get_test_config();
        let body = serde_json::json!({
            "prompt": "test prompt"
        });
        
        let req = Request::builder()
            .method("POST")
            .uri("http://test.com")
            .header("Content-Type", "application/json")
            .body(Full::new(Bytes::from(serde_json::to_vec(&body).unwrap())))
            .unwrap();
            
        let response = proxy(req, &config).await.unwrap();
        // ... rest of test code ...
    }
    
    // ... other test functions ...
} 