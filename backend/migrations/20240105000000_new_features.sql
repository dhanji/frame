-- Chat conversations and messages
CREATE TABLE IF NOT EXISTS chat_conversations (
    id TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL,
    title TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS chat_messages (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    role TEXT NOT NULL CHECK(role IN ('user', 'assistant')),
    content TEXT NOT NULL,
    tool_calls TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (conversation_id) REFERENCES chat_conversations(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_chat_conversations_user_id ON chat_conversations(user_id);
CREATE INDEX IF NOT EXISTS idx_chat_messages_conversation_id ON chat_messages(conversation_id);

-- Automations
CREATE TABLE IF NOT EXISTS automations (
    id TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    schedule TEXT NOT NULL,
    prompt TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    last_run DATETIME,
    next_run DATETIME,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS automation_runs (
    id TEXT PRIMARY KEY,
    automation_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('running', 'success', 'failed')),
    result TEXT,
    error TEXT,
    started_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    completed_at DATETIME,
    FOREIGN KEY (automation_id) REFERENCES automations(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_automations_user_id ON automations(user_id);
CREATE INDEX IF NOT EXISTS idx_automations_enabled ON automations(enabled);
CREATE INDEX IF NOT EXISTS idx_automation_runs_automation_id ON automation_runs(automation_id);

-- Reminders
CREATE TABLE IF NOT EXISTS reminders (
    id TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL,
    title TEXT NOT NULL,
    notes TEXT,
    due_date DATETIME,
    completed BOOLEAN NOT NULL DEFAULT FALSE,
    completed_at DATETIME,
    email_conversation_id TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_reminders_user_id ON reminders(user_id);
CREATE INDEX IF NOT EXISTS idx_reminders_completed ON reminders(completed);
CREATE INDEX IF NOT EXISTS idx_reminders_due_date ON reminders(due_date);

-- Calendar events
CREATE TABLE IF NOT EXISTS calendar_events (
    id TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL,
    calendar_id TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    location TEXT,
    start_time DATETIME NOT NULL,
    end_time DATETIME NOT NULL,
    all_day BOOLEAN NOT NULL DEFAULT FALSE,
    recurrence_rule TEXT,
    attendees TEXT,
    status TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_calendar_events_user_id ON calendar_events(user_id);
CREATE INDEX IF NOT EXISTS idx_calendar_events_start_time ON calendar_events(start_time);
CREATE INDEX IF NOT EXISTS idx_calendar_events_calendar_id ON calendar_events(calendar_id);

-- Money accounts
CREATE TABLE IF NOT EXISTS money_accounts (
    id TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL,
    account_type TEXT NOT NULL CHECK(account_type IN ('bank', 'cash_app', 'other')),
    account_name TEXT NOT NULL,
    balance REAL NOT NULL DEFAULT 0.0,
    currency TEXT NOT NULL DEFAULT 'USD',
    credentials TEXT,
    last_sync DATETIME,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS money_transactions (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL,
    transaction_date DATETIME NOT NULL,
    description TEXT NOT NULL,
    amount REAL NOT NULL,
    category TEXT,
    transaction_type TEXT NOT NULL CHECK(transaction_type IN ('income', 'expense', 'transfer')),
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (account_id) REFERENCES money_accounts(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_money_accounts_user_id ON money_accounts(user_id);
CREATE INDEX IF NOT EXISTS idx_money_transactions_account_id ON money_transactions(account_id);
CREATE INDEX IF NOT EXISTS idx_money_transactions_date ON money_transactions(transaction_date);
