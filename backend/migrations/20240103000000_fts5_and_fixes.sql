-- Add drafts table (missing from migrations)
CREATE TABLE IF NOT EXISTS drafts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    to_addresses TEXT,
    cc_addresses TEXT,
    bcc_addresses TEXT,
    subject TEXT,
    body_text TEXT,
    body_html TEXT,
    attachments TEXT,
    in_reply_to TEXT,
    email_references TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_drafts_user_id ON drafts(user_id);

-- Create FTS5 virtual table for full-text search
CREATE VIRTUAL TABLE IF NOT EXISTS emails_fts USING fts5(
    subject, 
    body_text, 
    from_address, 
    to_addresses,
    content='emails', 
    content_rowid='id'
);

-- Populate FTS5 table with existing data
INSERT INTO emails_fts(rowid, subject, body_text, from_address, to_addresses)
SELECT id, subject, COALESCE(body_text, ''), from_address, to_addresses
FROM emails;

-- Create triggers to keep FTS5 in sync with emails table
CREATE TRIGGER IF NOT EXISTS emails_ai AFTER INSERT ON emails BEGIN
  INSERT INTO emails_fts(rowid, subject, body_text, from_address, to_addresses)
  VALUES (new.id, new.subject, COALESCE(new.body_text, ''), new.from_address, new.to_addresses);
END;

CREATE TRIGGER IF NOT EXISTS emails_au AFTER UPDATE ON emails BEGIN
  UPDATE emails_fts SET 
    subject = new.subject,
    body_text = COALESCE(new.body_text, ''),
    from_address = new.from_address,
    to_addresses = new.to_addresses
  WHERE rowid = new.id;
END;

CREATE TRIGGER IF NOT EXISTS emails_ad AFTER DELETE ON emails BEGIN
  DELETE FROM emails_fts WHERE rowid = old.id;
END;

-- Create send queue table for offline support and retry logic
CREATE TABLE IF NOT EXISTS send_queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    to_addresses TEXT NOT NULL,
    cc_addresses TEXT,
    bcc_addresses TEXT,
    subject TEXT NOT NULL,
    body_text TEXT,
    body_html TEXT,
    attachments TEXT,
    in_reply_to TEXT,
    email_references TEXT,
    retry_count INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 3,
    last_error TEXT,
    status TEXT NOT NULL DEFAULT 'pending', -- pending, sending, failed, sent
    scheduled_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    sent_at DATETIME,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_send_queue_user_id ON send_queue(user_id);
CREATE INDEX IF NOT EXISTS idx_send_queue_status ON send_queue(status);
CREATE INDEX IF NOT EXISTS idx_send_queue_scheduled_at ON send_queue(scheduled_at);
