CREATE TABLE fuses (
    id              BIGSERIAL   PRIMARY KEY,
    name            TEXT        NOT NULL UNIQUE,
    flow_name       TEXT        NOT NULL,
    schedule_kind   TEXT        NOT NULL,
    schedule_spec   TEXT        NOT NULL,
    enabled         BOOLEAN     NOT NULL DEFAULT TRUE,
    last_run_at     TIMESTAMPTZ NULL,
    last_run_status TEXT        NULL,
    last_error      TEXT        NULL,
    next_run_at     TIMESTAMPTZ NOT NULL,
    run_count       BIGINT      NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_fuses_next_run_at ON fuses (next_run_at);
CREATE INDEX idx_fuses_enabled ON fuses (enabled);
