use anyhow::Context;
use bytes::Bytes;
use http_cache_reqwest::{CacheMode, HttpCacheOptions};
use reqwest_middleware::ClientWithMiddleware;
use std::sync::Arc;

pub fn cache_options() -> HttpCacheOptions {
    HttpCacheOptions {
        response_cache_mode_fn: Some(Arc::new(|_, response| {
            (200..300)
                .contains(&response.status)
                .then_some(CacheMode::ForceCache)
        })),
        ..Default::default()
    }
}

pub async fn get_with_cache_fallback(
    client: &ClientWithMiddleware,
    name: &str,
    url: &str,
) -> anyhow::Result<Bytes> {
    match get(client, url, CacheMode::Reload).await {
        Ok(contents) => Ok(contents),
        Err(download_error) => {
            let contents = get(client, url, CacheMode::OnlyIfCached)
                .await
                .with_context(|| {
                    format!("no cached {name} response after download failed: {download_error}")
                })?;
            warn!("{name} download failed, using cached response: {download_error}");
            Ok(contents)
        }
    }
}

async fn get(
    client: &ClientWithMiddleware,
    url: &str,
    cache_mode: CacheMode,
) -> anyhow::Result<Bytes> {
    let response = client.get(url).with_extension(cache_mode).send().await?;
    let response = response.error_for_status()?;
    Ok(response.bytes().await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_cache_reqwest::{Cache, HttpCache, RedbManager};
    use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::Path;
    use std::thread;

    fn spawn_server(responses: Vec<&'static str>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();

        thread::spawn(move || {
            for response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request_start = [0];
                stream.read_exact(&mut request_start).unwrap();
                stream.write_all(response.as_bytes()).unwrap();
            }
        });

        format!("http://{address}/data")
    }

    fn test_client(cache_path: &Path) -> ClientWithMiddleware {
        let manager = RedbManager::new(cache_path).unwrap();
        ClientBuilder::new(reqwest::Client::new())
            .with(Cache(HttpCache {
                mode: CacheMode::Default,
                manager,
                options: cache_options(),
            }))
            .build()
    }

    #[tokio::test]
    async fn downloads_current_response() {
        let response = "HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\ncurrent";
        let url = spawn_server(vec![response]);
        let cache_dir = tempfile::tempdir().unwrap();
        let cache_path = cache_dir.path().join("http-cache.redb");
        let client = test_client(&cache_path);

        let contents = get_with_cache_fallback(&client, "test data", &url)
            .await
            .unwrap();

        assert_eq!(contents, b"current".as_slice());
    }

    #[tokio::test]
    async fn falls_back_to_cached_response() {
        let current = "HTTP/1.1 200 OK\r\nContent-Length: 6\r\nCache-Control: no-store\r\nConnection: close\r\n\r\ncached";
        let failure =
            "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        let url = spawn_server(vec![current, failure]);
        let cache_dir = tempfile::tempdir().unwrap();
        let cache_path = cache_dir.path().join("http-cache.redb");
        let client = test_client(&cache_path);

        get_with_cache_fallback(&client, "test data", &url)
            .await
            .unwrap();
        drop(client);

        let client = test_client(&cache_path);
        let contents = get_with_cache_fallback(&client, "test data", &url)
            .await
            .unwrap();

        assert_eq!(contents, b"cached".as_slice());
    }

    #[tokio::test]
    async fn fails_without_cached_response() {
        let failure =
            "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        let url = spawn_server(vec![failure]);
        let cache_dir = tempfile::tempdir().unwrap();
        let cache_path = cache_dir.path().join("http-cache.redb");
        let client = test_client(&cache_path);

        let error = get_with_cache_fallback(&client, "test data", &url)
            .await
            .unwrap_err();

        assert!(error.to_string().contains("no cached test data response"));
    }
}
