CREATE TABLE users (
    id            BIGSERIAL PRIMARY KEY,
    email         TEXT      NOT NULL UNIQUE,
    password_hash TEXT      NOT NULL,
    role          TEXT      NOT NULL DEFAULT 'user' CHECK (role IN ('admin','user')),
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

INSERT INTO users (email, password_hash, role) VALUES
    ('admin', '$argon2id$v=19$m=16384,t=2,p=1$Y2F0YWJsYXN0YWRtaW4wMQ$48Sz0gH0b2FmBSf8meNbu6/2iA2Un9Oi0pN56uuyor4', 'admin');
