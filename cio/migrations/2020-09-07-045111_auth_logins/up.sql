CREATE TABLE auth_users (
    id SERIAL PRIMARY KEY,
    user_id VARCHAR NOT NULL UNIQUE,
    name VARCHAR NOT NULL,
    nickname VARCHAR NOT NULL,
    username VARCHAR NOT NULL,
    email VARCHAR NOT NULL,
    email_verified BOOLEAN NOT NULL DEFAULT 'f',
    picture VARCHAR NOT NULL,
    company VARCHAR NOT NULL,
    blog VARCHAR NOT NULL,
    phone VARCHAR NOT NULL,
    phone_verified BOOLEAN NOT NULL DEFAULT 'f',
    locale VARCHAR NOT NULL,
    login_provider VARCHAR NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    last_login TIMESTAMPTZ NOT NULL,
    last_application_accessed VARCHAR NOT NULL,
    last_ip VARCHAR NOT NULL,
    logins_count INTEGER NOT NULL,
    link_to_people TEXT [] NOT NULL,
    link_to_auth_user_logins TEXT [] NOT NULL
)
