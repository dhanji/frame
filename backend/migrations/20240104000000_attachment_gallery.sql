-- Add new fields to attachments table for gallery feature
ALTER TABLE attachments ADD COLUMN sender_email TEXT;
ALTER TABLE attachments ADD COLUMN sender_name TEXT;
ALTER TABLE attachments ADD COLUMN received_at DATETIME;
ALTER TABLE attachments ADD COLUMN source_account TEXT;
ALTER TABLE attachments ADD COLUMN keywords TEXT;
ALTER TABLE attachments ADD COLUMN preview_generated BOOLEAN NOT NULL DEFAULT FALSE;

-- Create indexes for gallery search and filtering
CREATE INDEX IF NOT EXISTS idx_attachments_filename ON attachments(filename);
CREATE INDEX IF NOT EXISTS idx_attachments_sender_email ON attachments(sender_email);
CREATE INDEX IF NOT EXISTS idx_attachments_received_at ON attachments(received_at);
CREATE INDEX IF NOT EXISTS idx_attachments_content_type ON attachments(content_type);
CREATE INDEX IF NOT EXISTS idx_attachments_size ON attachments(size);
CREATE INDEX IF NOT EXISTS idx_attachments_source_account ON attachments(source_account);

-- Composite indexes for common filter combinations
CREATE INDEX IF NOT EXISTS idx_attachments_user_date ON attachments(email_id, received_at DESC);
CREATE INDEX IF NOT EXISTS idx_attachments_user_sender ON attachments(email_id, sender_email);

-- Populate sender information from existing emails
UPDATE attachments 
SET 
    sender_email = (SELECT from_address FROM emails WHERE emails.id = attachments.email_id),
    sender_name = (SELECT from_address FROM emails WHERE emails.id = attachments.email_id),
    received_at = (SELECT date FROM emails WHERE emails.id = attachments.email_id),
    source_account = 'default'
WHERE email_id IS NOT NULL;

-- Set received_at for draft attachments to created_at
UPDATE attachments 
SET received_at = created_at
WHERE email_id IS NULL AND draft_id IS NOT NULL;
