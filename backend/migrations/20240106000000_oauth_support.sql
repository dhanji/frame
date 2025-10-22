-- Add OAuth support to users table
ALTER TABLE users ADD COLUMN oauth_provider TEXT;
ALTER TABLE users ADD COLUMN oauth_access_token TEXT;
ALTER TABLE users ADD COLUMN oauth_refresh_token TEXT;

-- Make password_hash optional for OAuth users
-- SQLite doesn't support ALTER COLUMN, so we'll handle this in the application logic
