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

INSERT INTO users (email, password_hash, role)
VALUES (
    'admin@admin.com',
    '$argon2id$v=19$m=65536,t=3,p=4$I+o759MGU1FYHOb2P9mgog$8qh2IgRssw4nBCbDOA96IWgj/pnTVFkQzs28qRtQLkE',
    'admin'
)
ON CONFLICT (email) DO NOTHING;
