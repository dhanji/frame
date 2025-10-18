export interface User {
    id: string;
    email: string;
}

export interface LoginRequest {
    email: string;
    password: string;
    imap_host: string;
    imap_port: number;
    smtp_host: string;
    smtp_port: number;
}

export interface LoginResponse {
    token: string;
    user: User;
}

export interface Email {
    id: string;
    user_id: string;
    message_id: string;
    thread_id: string;
    folder_id: string;
    subject: string;
    from_address: string;
    from_name?: string;
    to_addresses: string;
    cc_addresses?: string;
    bcc_addresses?: string;
    body_text?: string;
    body_html?: string;
    is_read: boolean;
    is_starred: boolean;
    has_attachments: boolean;
    date: string;
    created_at: string;
    updated_at: string;
}

export interface EmailPreview {
    id: string;
    from_address: string;
    from_name?: string;
    subject: string;
    preview_text: string;
    date: string;
    is_read: boolean;
    has_attachments: boolean;
}

export interface Conversation {
    thread_id: string;
    subject: string;
    participants: string[];
    last_message_date: string;
    message_count: number;
    unread_count: number;
    has_attachments: boolean;
    is_starred: boolean;
    id: string;
    preview: string;
    preview_messages: EmailPreview[];
}

export interface Folder {
    id: string;
    user_id: string;
    name: string;
    folder_type: string;
    parent_id?: string;
    unread_count: number;
    total_count: number;
    created_at: string;
    updated_at: string;
}

export interface SendEmailRequest {
    to: string[];
    cc?: string[];
    bcc?: string[];
    subject: string;
    body_text?: string;
    body_html?: string;
    attachments?: AttachmentUpload[];
}

export interface ReplyEmailRequest {
    email_id: string;
    reply_all: boolean;
    body_text?: string;
    body_html?: string;
    attachments?: AttachmentUpload[];
}

export interface SearchQuery {
    query: string;
    from?: string;
    to?: string;
    subject?: string;
    has_attachments?: boolean;
    is_unread?: boolean;
    date_from?: string;
    date_to?: string;
    folder_id?: string;
}

export interface Draft {
    id?: string | null;
    to: string[];
    cc?: string[];
    bcc?: string[];
    subject: string;
    body_text?: string;
    body_html?: string;
    attachments?: AttachmentUpload[];
    last_saved?: string;
}

export interface AttachmentUpload {
    filename: string;
    content_type: string;
    content: string; // Base64 encoded
    size?: number;
}

export interface FilterRule {
    id: string;
    name: string;
    conditions: FilterConditions;
    actions: FilterActions;
    is_active: boolean;
    priority: number;
}

export interface FilterConditions {
    from?: string;
    to?: string;
    subject?: string;
    body_contains?: string;
    has_attachments?: boolean;
    size_greater_than?: number;
    size_less_than?: number;
}

export interface FilterActions {
    move_to_folder?: string;
    mark_as_read?: boolean;
    mark_as_starred?: boolean;
    add_label?: string;
    forward_to?: string;
    delete?: boolean;
}

export interface UserSettings {
    theme: 'light' | 'dark';
    language: string;
    notifications_enabled: boolean;
    auto_mark_read: boolean;
    auto_mark_read_delay: number;
    conversation_view: 'expanded' | 'collapsed';
    emails_per_page: number;
    signature?: string;
    reply_position: 'top' | 'bottom';
    keyboard_shortcuts_enabled: boolean;
}