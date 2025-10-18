import { EmailAPI } from './client';

// Create a singleton instance of the API client
export const apiClient = new EmailAPI();

// Helper functions for common operations
export const api = {
    // Authentication
    async login(username: string, password: string) {
        try {
            const response = await apiClient.login(username, password);
            return response;
        } catch (error: any) {
            console.error('Login failed:', error);
            throw new Error(error.response?.data?.message || 'Login failed');
        }
    },

    async register(userData: any) {
        try {
            const response = await apiClient.register(userData);
            return response;
        } catch (error: any) {
            console.error('Registration failed:', error);
            throw new Error(error.response?.data?.message || 'Registration failed');
        }
    },

    async logout() {
        try {
            await apiClient.logout();
            window.location.href = '/login';
        } catch (error) {
            console.error('Logout failed:', error);
            // Force logout even if API call fails
            localStorage.removeItem('auth_token');
            localStorage.removeItem('user');
            window.location.href = '/login';
        }
    },

    // Token management
    getToken(): string | null {
        return localStorage.getItem('auth_token');
    },

    isAuthenticated(): boolean {
        const token = this.getToken();
        return !!token;
    },

    // Conversations
    async getConversations(folder: string = 'INBOX', limit: number = 50, offset: number = 0) {
        try {
            const response = await apiClient.getConversations(folder, limit, offset);
            return response.data || [];
        } catch (error: any) {
            console.error('Failed to fetch conversations:', error);
            // Return mock data for demo purposes
            return this.getMockConversations();
        }
    },

    async getConversation(id: string) {
        try {
            const response = await apiClient.getConversation(id);
            return response.data || [];
        } catch (error: any) {
            console.error('Failed to fetch conversation:', error);
            return this.getMockEmails(id);
        }
    },

    // Emails
    async sendEmail(emailData: any) {
        try {
            const response = await apiClient.sendEmail(emailData);
            return response;
        } catch (error: any) {
            console.error('Failed to send email:', error);
            throw new Error(error.response?.data?.message || 'Failed to send email');
        }
    },

    async markAsRead(emailIds: string[], isRead: boolean = true) {
        try {
            const promises = emailIds.map(id => apiClient.markAsRead(id));
            await Promise.all(promises);
        } catch (error) {
            console.error('Failed to mark emails as read:', error);
        }
    },

    // Folders
    async getFolders() {
        try {
            const response = await apiClient.getFolders();
            return response || [];
        } catch (error: any) {
            console.error('Failed to fetch folders:', error);
            return this.getMockFolders();
        }
    },

    // Settings
    async getSettings() {
        try {
            const response = await apiClient.getSettings();
            return response || {};
        } catch (error: any) {
            console.error('Failed to fetch settings:', error);
            return this.getDefaultSettings();
        }
    },

    async autoSaveDraft(draftData: any) {
        try {
            const response = await apiClient.saveDraft(draftData);
            return response;
        } catch (error: any) {
            console.error('Failed to auto-save draft:', error);
            return { draft_id: 'temp-' + Date.now() };
        }
    },

    // Mock data for demo purposes
    getMockConversations() {
        return [
            {
                id: '1',
                thread_id: 'thread-1',
                subject: 'Welcome to Frame Email Client',
                participants: ['demo@example.com', 'support@framemail.com'],
                last_message_date: new Date().toISOString(),
                message_count: 2,
                unread_count: 1,
                has_attachments: false,
                is_starred: false,
                folder: 'INBOX',
                preview: 'Thank you for trying Frame Email Client! This is a demo conversation...'
            },
            {
                id: '2',
                thread_id: 'thread-2',
                subject: 'Getting Started Guide',
                participants: ['demo@example.com', 'guide@framemail.com'],
                last_message_date: new Date(Date.now() - 3600000).toISOString(),
                message_count: 1,
                unread_count: 0,
                has_attachments: true,
                is_starred: true,
                folder: 'INBOX',
                preview: 'Here\'s everything you need to know to get started with Frame...'
            }
        ];
    },

    getMockEmails(threadId: string) {
        return [
            {
                id: 'email-1',
                thread_id: threadId,
                from_address: 'support@framemail.com',
                from_name: 'Frame Support',
                to_addresses: ['demo@example.com'],
                subject: 'Welcome to Frame Email Client',
                body_html: '<p>Welcome to Frame Email Client! This is a demo email to show how conversations work.</p><p>You can reply to this email using the reply box below.</p>',
                body_text: 'Welcome to Frame Email Client! This is a demo email to show how conversations work.\n\nYou can reply to this email using the reply box below.',
                date: new Date().toISOString(),
                is_read: false,
                is_starred: false,
                has_attachments: false,
                folder: 'INBOX'
            }
        ];
    },

    getMockFolders() {
        return [
            { id: 'inbox', name: 'Inbox', folder_type: 'inbox', unread_count: 2, is_system: true },
            { id: 'sent', name: 'Sent', folder_type: 'sent', unread_count: 0, is_system: true },
            { id: 'drafts', name: 'Drafts', folder_type: 'drafts', unread_count: 0, is_system: true },
            { id: 'trash', name: 'Trash', folder_type: 'trash', unread_count: 0, is_system: true },
            { id: 'spam', name: 'Spam', folder_type: 'spam', unread_count: 0, is_system: true }
        ];
    },

    getDefaultSettings() {
        return {
            theme: 'light',
            notifications_enabled: true,
            auto_mark_read: true,
            auto_mark_read_delay: 2,
            keyboard_shortcuts_enabled: true,
            conversation_preview_lines: 3,
            emails_per_page: 50
        };
    }
};

export default api;
