INSERT INTO users (username, email, first_name, last_name, password_hash, role, active, should_change_password, created_at, updated_at) VALUES
('admin', NULL, 'Admin', 'Admin', '$2b$12$XwVwGIL/g.wL2sTW3PSLc.DA3XBwCIFXdaydKUR.zlsEhBYVxmc8i', 'admin', TRUE, TRUE, EXTRACT(EPOCH FROM NOW()), EXTRACT(EPOCH FROM NOW()));

