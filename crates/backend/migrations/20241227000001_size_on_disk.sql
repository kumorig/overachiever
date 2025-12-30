-- App install size cache (community-sourced from desktop clients)
CREATE TABLE IF NOT EXISTS app_size_on_disk (
    appid BIGINT PRIMARY KEY,
    size_bytes BIGINT NOT NULL,
    reported_count INT NOT NULL DEFAULT 1,
    first_reported_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_reported_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for efficient lookups
CREATE INDEX IF NOT EXISTS idx_app_size_on_disk_appid ON app_size_on_disk(appid);

-- API request log for tracking public endpoint usage
CREATE TABLE IF NOT EXISTS api_request_log (
    id BIGSERIAL PRIMARY KEY,
    endpoint TEXT NOT NULL,
    client_ip TEXT,
    user_agent TEXT,
    referer TEXT,
    query_params TEXT,
    app_ids TEXT,  -- comma-separated list of requested app IDs
    requested_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for analytics queries
CREATE INDEX IF NOT EXISTS idx_api_request_log_endpoint ON api_request_log(endpoint);
CREATE INDEX IF NOT EXISTS idx_api_request_log_requested_at ON api_request_log(requested_at);
