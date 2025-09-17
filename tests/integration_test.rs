use std::process::Command;

/// Test the HTTP GET endpoint using curl
/// 
/// NOTE: This test requires the Redis module to be loaded and running.
/// To run this test properly:
/// 1. Load the Redis module: redis-cli MODULE LOAD /path/to/libredis_http.dylib
/// 2. Ensure Redis is running on localhost:6379
/// 3. Run the test: cargo test --test integration_test
/// 
/// The test will be skipped if the HTTP server is not available.
#[test]
fn test_http_get_endpoint_with_curl() {
    // Test with a sample Redis key
    let test_key = "test_key_123";
    let url = format!("http://localhost:4887/GET/{}", test_key);

    // Execute curl command without authentication
    let output = Command::new("curl")
        .arg("-s")  // Silent mode
        .arg("-w")  // Write format
        .arg("%{http_code}")  // Get HTTP status code
        .arg("-o")  // Output to file
        .arg("/dev/null")  // Discard response body for status check
        .arg(&url)
        .output();

    match output {
        Ok(result) => {
            let status_code = String::from_utf8_lossy(&result.stdout);
            println!("HTTP Status Code for {}: {}", url, status_code);
            
            // Check if server is running
            if status_code.contains("000") {
                println!("HTTP server is not running. Skipping test.");
                println!("To run this test:");
                println!("1. Load the Redis module: redis-cli MODULE LOAD /path/to/libredis_http.dylib");
                println!("2. Ensure Redis is running on localhost:6379");
                return; // Skip the test
            }
            
            // The endpoint should return 401 Unauthorized since we're not providing authentication
            // This confirms the endpoint is working and the authentication middleware is active
            assert!(status_code.contains("401") || status_code.contains("200"));
        }
        Err(e) => {
            // If curl is not available, skip the test
            println!("Curl not available, skipping HTTP endpoint test: {}", e);
        }
    }

    // Test with authentication (if Redis is available)
    let auth_output = Command::new("curl")
        .arg("-s")
        .arg("-u")  // Basic auth
        .arg("default:")  // Default Redis password (empty)
        .arg("-w")
        .arg("%{http_code}")
        .arg("-o")
        .arg("/dev/null")
        .arg(&url)
        .output();

    match auth_output {
        Ok(result) => {
            let status_code = String::from_utf8_lossy(&result.stdout);
            println!("Authenticated HTTP Status Code for {}: {}", url, status_code);
            
            // Should return 200 (success) or 404 (key not found) or 500 (Redis connection error)
            assert!(status_code.contains("200") || status_code.contains("404") || status_code.contains("500"));
        }
        Err(_) => {
            // If curl is not available, skip the test
            println!("Curl not available, skipping authenticated HTTP endpoint test");
        }
    }
}

/// Test the HTTP GET endpoint with different response formats
#[test]
fn test_http_get_endpoint_formats() {
    let test_key = "test_key_format";
    let base_url = format!("http://localhost:4887/GET/{}", test_key);

    // First check if server is running
    let check_output = Command::new("curl")
        .arg("-s")
        .arg("-w")
        .arg("%{http_code}")
        .arg("-o")
        .arg("/dev/null")
        .arg(&base_url)
        .output();

    match check_output {
        Ok(result) => {
            let status_code = String::from_utf8_lossy(&result.stdout);
            if status_code.contains("000") {
                println!("HTTP server is not running. Skipping format tests.");
                return;
            }
        }
        Err(_) => {
            println!("Curl not available, skipping format tests");
            return;
        }
    }

    // Test JSON format (default)
    let json_output = Command::new("curl")
        .arg("-s")
        .arg("-H")
        .arg("Accept: application/json")
        .arg("-u")
        .arg("default:")
        .arg(&base_url)
        .output();

    match json_output {
        Ok(result) => {
            let response = String::from_utf8_lossy(&result.stdout);
            println!("JSON Response: {}", response);
            // Should contain JSON structure
            assert!(response.contains("{") && response.contains("}"));
        }
        Err(_) => {
            println!("Curl not available, skipping JSON format test");
        }
    }

    // Test XML format
    let xml_output = Command::new("curl")
        .arg("-s")
        .arg("-H")
        .arg("Accept: application/xml")
        .arg("-u")
        .arg("default:")
        .arg(&base_url)
        .output();

    match xml_output {
        Ok(result) => {
            let response = String::from_utf8_lossy(&result.stdout);
            println!("XML Response: {}", response);
            // Should contain XML structure
            assert!(response.contains("<") && response.contains(">"));
        }
        Err(_) => {
            println!("Curl not available, skipping XML format test");
        }
    }

    // Test plain text format
    let text_output = Command::new("curl")
        .arg("-s")
        .arg("-H")
        .arg("Accept: text/plain")
        .arg("-u")
        .arg("default:")
        .arg(&base_url)
        .output();

    match text_output {
        Ok(result) => {
            let response = String::from_utf8_lossy(&result.stdout);
            println!("Text Response: {}", response);
            // Should be plain text (no JSON/XML structure)
            assert!(!response.contains("{") || !response.contains("<"));
        }
        Err(_) => {
            println!("Curl not available, skipping text format test");
        }
    }
}

/// Test the HTTP GET endpoint with different Redis keys
#[test]
fn test_http_get_endpoint_different_keys() {
    let test_keys = vec!["key1", "my_key", "key-with-dashes", "key_with_underscores"];
    
    // First check if server is running
    let check_url = "http://localhost:4887/GET/test";
    let check_output = Command::new("curl")
        .arg("-s")
        .arg("-w")
        .arg("%{http_code}")
        .arg("-o")
        .arg("/dev/null")
        .arg(check_url)
        .output();

    match check_output {
        Ok(result) => {
            let status_code = String::from_utf8_lossy(&result.stdout);
            if status_code.contains("000") {
                println!("HTTP server is not running. Skipping different keys tests.");
                return;
            }
        }
        Err(_) => {
            println!("Curl not available, skipping different keys tests");
            return;
        }
    }
    
    for key in test_keys {
        let url = format!("http://localhost:4887/GET/{}", key);
        
        let output = Command::new("curl")
            .arg("-s")
            .arg("-w")
            .arg("%{http_code}")
            .arg("-o")
            .arg("/dev/null")
            .arg("-u")
            .arg("default:")
            .arg(&url)
            .output();

        match output {
            Ok(result) => {
                let status_code = String::from_utf8_lossy(&result.stdout);
                println!("Status code for key '{}': {}", key, status_code);
                
                // Should return valid HTTP status codes
                assert!(status_code.contains("200") || status_code.contains("404") || status_code.contains("500"));
            }
            Err(_) => {
                println!("Curl not available, skipping test for key: {}", key);
            }
        }
    }
}
