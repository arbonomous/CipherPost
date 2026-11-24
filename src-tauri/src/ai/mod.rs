use std::time::{Duration, Instant};

use reqwest::Client;
use serde_json::{json, Value};
use url::Url;

use crate::error::{CipherError, CipherResult};
use crate::models::{AiDraftResponse, AiHealthResponse, AiSummaryResponse};

/// Zero-trust loopback guard: guarantees CipherPost NEVER transmits email content
/// to any non-local AI endpoint. Only `localhost`, `127.0.0.1`, and `[::1]` are permitted.
pub fn validate_local_endpoint(endpoint: &str) -> CipherResult<Url> {
    let parsed = Url::parse(endpoint.trim())
        .map_err(|e| CipherError::Validation(format!("Invalid local AI endpoint URL: {e}")))?;

    let host = parsed
        .host_str()
        .ok_or_else(|| CipherError::Validation("Local AI endpoint missing host".to_string()))?;

    let is_loopback = matches!(
        host.to_ascii_lowercase().as_str(),
        "localhost" | "127.0.0.1" | "::1" | "[::1]"
    );

    if !is_loopback {
        return Err(CipherError::Validation(format!(
            "Zero-Trust Policy Violation: AI endpoint '{host}' is not local loopback (localhost / 127.0.0.1). External cloud inference is blocked."
        )));
    }

    Ok(parsed)
}

fn http_client() -> CipherResult<Client> {
    Client::builder()
        .timeout(Duration::from_secs(45))
        .build()
        .map_err(|e| CipherError::Ai(e.to_string()))
}

/// Checks whether a local Ollama (`11434`) or MLX OpenAI-compatible server is running
/// and lists locally available models.
pub async fn check_local_ai_health(endpoint: &str) -> CipherResult<AiHealthResponse> {
    let base = validate_local_endpoint(endpoint)?;
    let client = http_client()?;
    let t0 = Instant::now();

    // Try Ollama `/api/tags` first
    let ollama_url = format!("{}/api/tags", base.as_str().trim_end_matches('/'));
    if let Ok(resp) = client.get(&ollama_url).send().await {
        if resp.status().is_success() {
            if let Ok(json_val) = resp.json::<Value>().await {
                let models = json_val["models"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                return Ok(AiHealthResponse {
                    reachable: true,
                    endpoint: endpoint.to_string(),
                    available_models: models,
                    latency_ms: t0.elapsed().as_millis() as u64,
                    error: None,
                });
            }
        }
    }

    // Fallback: MLX / OpenAI-compatible `/v1/models`
    let mlx_url = format!("{}/v1/models", base.as_str().trim_end_matches('/'));
    match client.get(&mlx_url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let json_val = resp.json::<Value>().await.unwrap_or_default();
            let models = json_val["data"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            Ok(AiHealthResponse {
                reachable: true,
                endpoint: endpoint.to_string(),
                available_models: models,
                latency_ms: t0.elapsed().as_millis() as u64,
                error: None,
            })
        }
        Ok(resp) => Ok(AiHealthResponse {
            reachable: false,
            endpoint: endpoint.to_string(),
            available_models: vec![],
            latency_ms: t0.elapsed().as_millis() as u64,
            error: Some(format!("Local endpoint returned HTTP {}", resp.status())),
        }),
        Err(e) => Ok(AiHealthResponse {
            reachable: false,
            endpoint: endpoint.to_string(),
            available_models: vec![],
            latency_ms: t0.elapsed().as_millis() as u64,
            error: Some(format!(
                "No local Ollama or MLX server detected at {endpoint}: {e}"
            )),
        }),
    }
}

async fn invoke_local_chat(
    endpoint: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
) -> CipherResult<String> {
    let base = validate_local_endpoint(endpoint)?;
    let client = http_client()?;
    let trimmed = base.as_str().trim_end_matches('/');

    // Attempt Ollama `/api/chat`
    let ollama_resp = client
        .post(format!("{trimmed}/api/chat"))
        .json(&json!({
            "model": model,
            "stream": false,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": user_prompt }
            ]
        }))
        .send()
        .await;

    if let Ok(resp) = ollama_resp {
        if resp.status().is_success() {
            let body: Value = resp
                .json()
                .await
                .map_err(|e| CipherError::Ai(format!("Invalid Ollama response: {e}")))?;
            if let Some(content) = body["message"]["content"].as_str() {
                return Ok(content.trim().to_string());
            }
        }
    }

    // Fallback: MLX / OpenAI-compatible `/v1/chat/completions`
    let mlx_resp = client
        .post(format!("{trimmed}/v1/chat/completions"))
        .json(&json!({
            "model": model,
            "temperature": 0.2,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": user_prompt }
            ]
        }))
        .send()
        .await
        .map_err(|e| {
            CipherError::Ai(format!(
                "Could not reach local Ollama/MLX server at {endpoint}: {e}"
            ))
        })?;

    if !mlx_resp.status().is_success() {
        return Err(CipherError::Ai(format!(
            "Local AI server returned HTTP {}",
            mlx_resp.status()
        )));
    }

    let body: Value = mlx_resp
        .json()
        .await
        .map_err(|e| CipherError::Ai(format!("Invalid MLX JSON response: {e}")))?;

    body["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.trim().to_string())
        .ok_or_else(|| CipherError::Ai("Local AI response missing assistant message".to_string()))
}

/// Generates a strict 2-bullet TL;DR triage summary on-device via Ollama or MLX.
pub async fn summarize_thread_local(
    endpoint: &str,
    model: &str,
    thread_id: &str,
    subject: &str,
    transcript: &str,
) -> CipherResult<AiSummaryResponse> {
    let t0 = Instant::now();
    let system_prompt = "You are CipherPost's local-only email triage assistant. Summarize the email thread into EXACTLY two crisp, high-signal bullet points (each under 22 words). Start each bullet with '- '. No preamble, no markdown headers.";
    let user_prompt = format!("Subject: {subject}\n\nThread:\n{transcript}");

    let raw = invoke_local_chat(endpoint, model, system_prompt, &user_prompt).await?;

    let mut bullets: Vec<String> = raw
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(|l| {
            l.trim_start_matches('-')
                .trim_start_matches('•')
                .trim_start_matches('*')
                .trim()
                .to_string()
        })
        .filter(|l| !l.is_empty())
        .take(2)
        .collect();

    if bullets.is_empty() {
        bullets.push(raw.clone());
    }

    let lower = raw.to_ascii_lowercase();
    let action_required = lower.contains("action")
        || lower.contains("reply")
        || lower.contains("approve")
        || lower.contains("confirm")
        || lower.contains("deadline")
        || lower.contains("review");

    Ok(AiSummaryResponse {
        thread_id: thread_id.to_string(),
        bullets,
        action_required,
        model_used: model.to_string(),
        latency_ms: t0.elapsed().as_millis() as u64,
    })
}

/// Drafts a concise, context-aware email reply strictly on-device via Ollama or MLX.
pub async fn draft_reply_local(
    endpoint: &str,
    model: &str,
    subject: &str,
    transcript: &str,
    instruction: Option<&str>,
) -> CipherResult<AiDraftResponse> {
    let t0 = Instant::now();
    let guidance = instruction
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("Write a concise, clear, direct reply.");
    let system_prompt = "You are CipherPost's local-only executive email composer. Write only the plain-text email reply body (no subject line, no placeholders unless necessary). Keep it concise, tactical, and professional.";
    let user_prompt = format!(
        "Subject: {subject}\nInstruction: {guidance}\n\nConversation Context:\n{transcript}"
    );

    let draft_text = invoke_local_chat(endpoint, model, system_prompt, &user_prompt).await?;

    Ok(AiDraftResponse {
        draft_text,
        model_used: model.to_string(),
        latency_ms: t0.elapsed().as_millis() as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enforces_strict_loopback_only_ai_endpoints() {
        assert!(validate_local_endpoint("http://localhost:11434").is_ok());
        assert!(validate_local_endpoint("http://127.0.0.1:8080/v1").is_ok());
        assert!(validate_local_endpoint("http://[::1]:11434").is_ok());

        // Cloud endpoints MUST be blocked
        assert!(validate_local_endpoint("https://api.openai.com/v1").is_err());
        assert!(validate_local_endpoint("https://evil.example.com:11434").is_err());
    }
}
