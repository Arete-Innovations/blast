CREATE TABLE sessions (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash BYTEA NOT NULL UNIQUE,
    user_agent TEXT,
    ip TEXT,
    revoked BOOLEAN NOT NULL DEFAULT FALSE,
    created_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM NOW())),
    last_seen_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM NOW())),
    expires_at BIGINT NOT NULL
);

CREATE INDEX sessions_token_hash_idx ON sessions (token_hash) WHERE NOT revoked;
CREATE INDEX sessions_user_id_idx ON sessions (user_id);
CREATE INDEX sessions_expires_at_idx ON sessions (expires_at) WHERE NOT revoked;
