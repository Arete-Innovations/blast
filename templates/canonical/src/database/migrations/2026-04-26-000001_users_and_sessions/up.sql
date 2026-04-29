CREATE TYPE user_role AS ENUM ('admin', 'member');

CREATE TABLE users (
    id            BIGSERIAL PRIMARY KEY,
    email         TEXT      NOT NULL UNIQUE,
    password_hash TEXT      NOT NULL,
    role          user_role NOT NULL DEFAULT 'member',
    created_at    BIGINT    NOT NULL DEFAULT extract(epoch from NOW())::bigint,
    updated_at    BIGINT    NOT NULL DEFAULT extract(epoch from NOW())::bigint,
    deleted_at    BIGINT    NULL
);

CREATE INDEX idx_users_email ON users (email);

CREATE TABLE sessions (
    id         BIGSERIAL PRIMARY KEY,
    user_id    BIGINT    NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token      TEXT      NOT NULL UNIQUE,
    expires_at BIGINT    NOT NULL,
    created_at BIGINT    NOT NULL DEFAULT extract(epoch from NOW())::bigint
);

CREATE INDEX idx_sessions_token   ON sessions (token);
CREATE INDEX idx_sessions_user_id ON sessions (user_id);
